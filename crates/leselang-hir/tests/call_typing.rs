use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leselang_hir::call_typing::{
    CallTypeError, CallTypeHost, CallTypeLimits, MAX_CALL_ARGUMENTS, check_call_arguments,
    infer_call_type,
};
use leselang_hir::ir::{Computation, ComputedArgument, GroupKind};
use leselang_hir::pure_typing::{
    MAX_TYPE_INFERENCE_DEPTH, PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
};
use leselang_runtime_core::{
    ArgumentTypeError, BinaryOperator, NamedArgumentError, NamedParameter, OperationCatalog,
    OperationCatalogError, OperationCatalogLimits, OperationSchema, ScalarArgumentDomain,
    ScalarType, ScalarTypeSet, ScalarValue, StringListValue,
};

type Ir = Computation<u8, u32, Rc<()>, ()>;
const LIMITS: CallTypeLimits = CallTypeLimits {
    pure: TypeInferenceLimits {
        max_nodes: 1024,
        max_depth: 16,
        max_bindings: 16,
    },
    max_arguments: MAX_CALL_ARGUMENTS,
};

#[derive(Default)]
struct Environment {
    queries: Cell<usize>,
    panic: Cell<bool>,
}
impl PureTypeEnvironment<u8, u32> for Environment {
    type Result = Rc<u8>;
    fn field_type(&self, result: &Rc<u8>, field: &u8) -> Option<ScalarType> {
        self.queries.set(self.queries.get() + 1);
        assert!(!self.panic.get(), "private native query");
        match (**result, *field) {
            (1, 7) => Some(ScalarType::Integer),
            (1, 8) => Some(ScalarType::String),
            _ => None,
        }
    }
    fn member_result(&self, group: &Rc<u8>, name: &str, operation: &u32) -> Option<Rc<u8>> {
        self.queries.set(self.queries.get() + 1);
        (**group == 2 && name == "move" && *operation == 17).then(|| Rc::new(1))
    }
}

fn literal(value: ScalarValue) -> Ir {
    Ir::Literal { value }
}
fn number(value: u64) -> Ir {
    literal(ScalarValue::Integer(value))
}
fn text(value: &str) -> Ir {
    literal(ScalarValue::String(value.into()))
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(name: &str, key: u8) -> Ir {
    Ir::Field {
        value: Box::new(local(name)),
        field: key,
    }
}
fn argument(name: &str, value: Ir) -> ComputedArgument<Ir> {
    ComputedArgument {
        name: name.into(),
        value,
    }
}
fn call(arguments: Vec<ComputedArgument<Ir>>) -> Ir {
    Ir::Call {
        operation: 17,
        arguments,
    }
}
fn choose(then: Ir, otherwise: Ir) -> Ir {
    Ir::Choose {
        when: Box::new(literal(ScalarValue::Boolean(true))),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn integer_domain() -> ScalarTypeSet {
    ScalarTypeSet::only(ScalarType::Integer)
}

#[test]
fn catalog_call_infers_real_projection_and_returns_original_result_declaration() {
    let parameters = [NamedParameter::required("position", integer_domain())];
    struct Receipt(Rc<()>);
    let owner = Rc::new(());
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: Receipt(owner.clone()),
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(
        9,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let environment = Environment::default();
    let prefix = [("receipt", PureType::Result(Rc::new(1)))];
    let expression = call(vec![argument(
        "position",
        Ir::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(field("receipt", 7)),
            right: Box::new(number(1)),
        },
    )]);
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let result = infer_call_type(&expression, &prefix, &host, LIMITS).unwrap();
    assert!(std::ptr::eq(result, &schemas[0].result));
    assert!(Rc::ptr_eq(&result.0, &owner));
    assert_eq!(Rc::strong_count(&owner), 2);
    assert_eq!(environment.queries.get(), 1);
    assert_eq!(
        Rc::strong_count(match &prefix[0].1 {
            PureType::Result(r) => r,
            _ => unreachable!(),
        }),
        1
    );
}

#[test]
fn exact_version_operation_and_grants_precede_type_queries() {
    let parameters = [NamedParameter::required("position", integer_domain())];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: (),
        required_capability: 31,
    }];
    let catalog = OperationCatalog::new(
        9,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let environment = Environment::default();
    let prefix = [("receipt", PureType::Result(Rc::new(1)))];
    let expression = call(vec![argument("position", field("receipt", 7))]);
    for (version, grants, error) in [
        (8, vec![31], OperationCatalogError::UnsupportedVersion),
        (9, vec![], OperationCatalogError::CapabilityDenied),
    ] {
        let host = CallTypeHost {
            catalog: &catalog,
            version,
            granted: &grants,
            environment: &environment,
        };
        assert_eq!(
            infer_call_type(&expression, &prefix, &host, LIMITS),
            Err(CallTypeError::Catalog(error))
        );
    }
    let unknown = Ir::Call {
        operation: 99,
        arguments: vec![argument("position", field("receipt", 7))],
    };
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    assert_eq!(
        infer_call_type(&unknown, &prefix, &host, LIMITS),
        Err(CallTypeError::Catalog(
            OperationCatalogError::UnknownOperation
        ))
    );
    assert_eq!(environment.queries.get(), 0);
}

#[test]
fn entire_argument_forest_precedes_catalog_comparisons_and_metadata_cloning() {
    struct Key;
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            panic!("private native comparison")
        }
    }
    struct Metadata;
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            panic!("private native clone")
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, _: &Self) -> bool {
            panic!("private type comparison")
        }
    }
    struct NativeEnvironment;
    impl PureTypeEnvironment<(), Key> for NativeEnvironment {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &()) -> Option<ScalarType> {
            panic!("private native field")
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &Key) -> Option<Metadata> {
            panic!("private native member")
        }
    }
    type NativeIr = Computation<(), Key, Rc<()>, ()>;
    let parameters = [
        NamedParameter::required("first", integer_domain()),
        NamedParameter::required("last", integer_domain()),
    ];
    let schemas = [OperationSchema {
        key: Key,
        parameters: &parameters,
        result: (),
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(
        1,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 2,
        },
    )
    .unwrap();
    let expression = NativeIr::Call {
        operation: Key,
        arguments: vec![
            ComputedArgument {
                name: "first".into(),
                value: NativeIr::Local {
                    name: "receipt".into(),
                },
            },
            ComputedArgument {
                name: "last".into(),
                value: NativeIr::Choose {
                    when: Box::new(NativeIr::Literal {
                        value: ScalarValue::Boolean(true),
                    }),
                    then: Box::new(NativeIr::Literal {
                        value: ScalarValue::Integer(1),
                    }),
                    otherwise: Box::new(NativeIr::Host {
                        effect: Box::new(Rc::new(())),
                    }),
                },
            },
        ],
    };
    let host = CallTypeHost {
        catalog: &catalog,
        version: 1,
        granted: &[()],
        environment: &NativeEnvironment,
    };
    assert!(matches!(
        infer_call_type(
            &expression,
            &[("receipt", PureType::Result(Metadata))],
            &host,
            LIMITS
        ),
        Err(CallTypeError::Inference {
            argument_index: 1,
            error: PureTypeError::Impure
        })
    ));
}

#[test]
fn aggregate_call_node_and_depth_budgets_are_inclusive_not_per_argument() {
    let parameters = [
        NamedParameter::required("a", integer_domain()),
        NamedParameter::required("b", integer_domain()),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let arguments = [argument("a", number(1)), argument("b", number(2))];
    let exact = CallTypeLimits {
        pure: TypeInferenceLimits {
            max_nodes: 3,
            max_depth: 1,
            max_bindings: 0,
        },
        max_arguments: 2,
    };
    assert!(check_call_arguments(&arguments, &schema, &[], &Environment::default(), exact).is_ok());
    for (limits, expected) in [
        (
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 2,
                    ..exact.pure
                },
                ..exact
            },
            1,
        ),
        (
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_depth: 0,
                    ..exact.pure
                },
                ..exact
            },
            0,
        ),
    ] {
        assert!(
            matches!(check_call_arguments(&arguments, &schema, &[], &Environment::default(), limits),
            Err(CallTypeError::Inference { argument_index, error: PureTypeError::Structure(_) }) if argument_index == expected)
        );
    }
    assert!(matches!(
        check_call_arguments(
            &arguments,
            &schema,
            &[],
            &Environment::default(),
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 0,
                    ..exact.pure
                },
                ..exact
            }
        ),
        Err(CallTypeError::Structure(_))
    ));
}

#[test]
fn zero_argument_policy_admits_only_parameterless_calls_with_root_budget() {
    let schema: OperationSchema<'_, (), &str, ScalarTypeSet, (), ()> = OperationSchema {
        key: (),
        parameters: &[],
        result: (),
        required_capability: (),
    };
    let exact = CallTypeLimits {
        pure: TypeInferenceLimits {
            max_nodes: 1,
            max_depth: 0,
            max_bindings: 0,
        },
        max_arguments: 0,
    };
    assert!(
        check_call_arguments::<u8, u32, Rc<()>, (), _, _, _, _, _>(
            &[],
            &schema,
            &[],
            &Environment::default(),
            exact
        )
        .is_ok()
    );
    assert_eq!(
        check_call_arguments(
            &[argument("a", number(1))],
            &schema,
            &[],
            &Environment::default(),
            exact
        ),
        Err(CallTypeError::ArgumentLimit)
    );
}

#[test]
fn all_fixed_limit_ceilings_scope_names_and_argument_names_are_checked() {
    let parameters = [NamedParameter::required("a", integer_domain())];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let arguments = [argument("a", number(1))];
    assert_eq!(
        check_call_arguments(
            &arguments,
            &schema,
            &[],
            &Environment::default(),
            CallTypeLimits {
                max_arguments: MAX_CALL_ARGUMENTS + 1,
                ..LIMITS
            }
        ),
        Err(CallTypeError::InvalidLimits)
    );
    assert_eq!(
        check_call_arguments(
            &arguments,
            &schema,
            &[],
            &Environment::default(),
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_depth: MAX_TYPE_INFERENCE_DEPTH + 1,
                    ..LIMITS.pure
                },
                ..LIMITS
            }
        ),
        Err(CallTypeError::Scope(PureTypeError::InvalidLimits))
    );
    for scope in [
        vec![("private-invalid", PureType::Scalar(ScalarType::Integer))],
        vec![
            ("a", PureType::Scalar(ScalarType::Integer)),
            ("a", PureType::Scalar(ScalarType::Integer)),
        ],
    ] {
        assert_eq!(
            check_call_arguments(&arguments, &schema, &scope, &Environment::default(), LIMITS),
            Err(CallTypeError::Scope(PureTypeError::InvalidScope))
        );
    }
    for name in ["", "9bad", "private-value", "\u{00e9}"] {
        assert_eq!(
            check_call_arguments(
                &[argument(name, number(1))],
                &schema,
                &[],
                &Environment::default(),
                LIMITS
            ),
            Err(CallTypeError::InvalidArgumentName { argument_index: 0 })
        );
    }
    let long = "a".repeat(65);
    assert_eq!(
        check_call_arguments(
            &[argument(&long, number(1))],
            &schema,
            &[],
            &Environment::default(),
            LIMITS
        ),
        Err(CallTypeError::InvalidArgumentName { argument_index: 0 })
    );
}

struct Domain<'a> {
    label: &'static str,
    events: &'a RefCell<Vec<String>>,
    accept: bool,
}
impl ScalarArgumentDomain for Domain<'_> {
    fn scalar_types(&self) -> ScalarTypeSet {
        self.events
            .borrow_mut()
            .push(format!("{}:type", self.label));
        integer_domain()
    }
    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        self.events
            .borrow_mut()
            .push(format!("{}:literal", self.label));
        self.accept
    }
}

#[test]
fn all_named_shape_errors_precede_domain_methods() {
    let events = RefCell::new(vec![]);
    let parameters = [
        NamedParameter::required(
            "a",
            Domain {
                label: "a",
                events: &events,
                accept: true,
            },
        ),
        NamedParameter::optional(
            "b",
            Domain {
                label: "b",
                events: &events,
                accept: true,
            },
        ),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    for (arguments, expected) in [
        (
            vec![argument("a", number(1)), argument("a", number(2))],
            NamedArgumentError::DuplicateArgument { index: 1 },
        ),
        (
            vec![argument("private", number(1))],
            NamedArgumentError::UnknownArgument { index: 0 },
        ),
        (
            vec![argument("b", number(1))],
            NamedArgumentError::MissingArgument { parameter_index: 0 },
        ),
        (
            vec![
                argument("a", number(1)),
                argument("b", number(1)),
                argument("extra", number(1)),
            ],
            NamedArgumentError::TooManyArguments,
        ),
    ] {
        assert_eq!(
            check_call_arguments(&arguments, &schema, &[], &Environment::default(), LIMITS),
            Err(CallTypeError::Names(expected))
        );
    }
    assert!(events.borrow().is_empty());
}

#[test]
fn declaration_order_and_original_positions_survive_omitted_optional_parameters() {
    let events = RefCell::new(vec![]);
    let parameters = [
        NamedParameter::optional(
            "unused",
            Domain {
                label: "unused",
                events: &events,
                accept: false,
            },
        ),
        NamedParameter::required(
            "first",
            Domain {
                label: "first",
                events: &events,
                accept: true,
            },
        ),
        NamedParameter::required(
            "last",
            Domain {
                label: "last",
                events: &events,
                accept: false,
            },
        ),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let arguments = [argument("last", number(2)), argument("first", number(1))];
    assert_eq!(
        check_call_arguments(&arguments, &schema, &[], &Environment::default(), LIMITS),
        Err(CallTypeError::Argument {
            parameter_index: 2,
            argument_index: 0,
            error: ArgumentTypeError::InvalidLiteralDomain
        })
    );
    assert_eq!(
        *events.borrow(),
        ["first:type", "first:literal", "last:type", "last:literal"]
    );
}

#[test]
fn selected_parameter_count_is_bounded_before_domains_or_inference() {
    let events = RefCell::new(vec![]);
    let parameters = [NamedParameter::required(
        "a",
        Domain {
            label: "a",
            events: &events,
            accept: true,
        },
    )];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    assert_eq!(
        check_call_arguments::<u8, u32, Rc<()>, (), _, _, _, _, _>(
            &[],
            &schema,
            &[],
            &Environment::default(),
            CallTypeLimits {
                max_arguments: 0,
                ..LIMITS
            }
        ),
        Err(CallTypeError::ParameterLimit)
    );
    assert!(events.borrow().is_empty());
}

#[test]
fn cold_type_errors_and_zero_iteration_bodies_are_inferred_not_stamped() {
    let parameters = [NamedParameter::required("position", integer_domain())];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let zero_loop = Ir::Loop {
        name: "n".into(),
        initial: Box::new(number(0)),
        condition: Box::new(literal(ScalarValue::Boolean(false))),
        next: Box::new(text("wrong")),
        limit: 0,
    };
    let zero_fold = Ir::Fold {
        name: "n".into(),
        item: "entry".into(),
        items: Box::new(literal(ScalarValue::StringList(StringListValue(vec![])))),
        initial: Box::new(number(0)),
        next: Box::new(text("wrong")),
        limit: 0,
    };
    for (value, expected) in [
        (choose(number(1), text("wrong")), PureTypeError::BranchTypes),
        (zero_loop, PureTypeError::LoopState),
        (zero_fold, PureTypeError::FoldState),
        (
            Ir::Recover {
                value: Box::new(number(1)),
                fallback: Box::new(text("wrong")),
            },
            PureTypeError::RecoveryTypes,
        ),
    ] {
        assert_eq!(
            check_call_arguments(
                &[argument("position", value)],
                &schema,
                &[],
                &Environment::default(),
                LIMITS
            ),
            Err(CallTypeError::Inference {
                argument_index: 0,
                error: expected
            })
        );
    }
}

#[test]
fn external_results_unknown_locals_and_bounded_literals_do_not_bypass_scalar_domains() {
    let parameters = [NamedParameter::required("position", integer_domain())];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let prefix = [("receipt", PureType::Result(Rc::new(1)))];
    assert_eq!(
        check_call_arguments(
            &[argument("position", local("receipt"))],
            &schema,
            &prefix,
            &Environment::default(),
            LIMITS
        ),
        Err(CallTypeError::Argument {
            parameter_index: 0,
            argument_index: 0,
            error: ArgumentTypeError::NonScalar
        })
    );
    assert_eq!(
        check_call_arguments(
            &[argument("position", local("missing"))],
            &schema,
            &[],
            &Environment::default(),
            LIMITS
        ),
        Err(CallTypeError::Inference {
            argument_index: 0,
            error: PureTypeError::UnknownLocal
        })
    );
    assert_eq!(
        check_call_arguments(
            &[argument("position", text(&"x".repeat(4097)))],
            &schema,
            &[],
            &Environment::default(),
            LIMITS
        ),
        Err(CallTypeError::Inference {
            argument_index: 0,
            error: PureTypeError::UnboundedLiteral
        })
    );
    assert_eq!(
        check_call_arguments(
            &[argument("position", text("7"))],
            &schema,
            &[],
            &Environment::default(),
            LIMITS
        ),
        Err(CallTypeError::Argument {
            parameter_index: 0,
            argument_index: 0,
            error: ArgumentTypeError::TypeMismatch
        })
    );
}

#[test]
fn dynamic_values_are_typed_without_pretending_the_native_value_domain_was_checked() {
    let events = RefCell::new(vec![]);
    let parameters = [NamedParameter::required(
        "position",
        Domain {
            label: "position",
            events: &events,
            accept: false,
        },
    )];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let dynamic = Ir::Binary {
        operator: BinaryOperator::Div,
        left: Box::new(number(1)),
        right: Box::new(number(0)),
    };
    assert!(
        check_call_arguments(
            &[argument("position", dynamic)],
            &schema,
            &[],
            &Environment::default(),
            LIMITS
        )
        .is_ok()
    );
    assert_eq!(*events.borrow(), ["position:type"]);
}

#[test]
fn only_call_roots_are_admitted_and_nested_effects_remain_impure() {
    let schema: [OperationSchema<'_, u32, &str, ScalarTypeSet, (), ()>; 1] = [OperationSchema {
        key: 17,
        parameters: &[],
        result: (),
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(
        1,
        &schema,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 1,
        granted: &[()],
        environment: &environment,
    };
    for expression in [
        number(1),
        Ir::Host {
            effect: Box::new(Rc::new(())),
        },
        Ir::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![],
        },
        choose(call(vec![]), call(vec![])),
        Ir::Bind {
            name: "x".into(),
            value: Box::new(number(1)),
            body: Box::new(call(vec![])),
        },
    ] {
        assert_eq!(
            infer_call_type(&expression, &[], &host, LIMITS),
            Err(CallTypeError::NotCall)
        );
    }
    assert_eq!(
        infer_call_type(&call(vec![argument("a", call(vec![]))]), &[], &host, LIMITS),
        Err(CallTypeError::Inference {
            argument_index: 0,
            error: PureTypeError::Impure
        })
    );
}

#[test]
fn native_query_unwind_releases_temporary_bindings_without_mutating_caller_scope() {
    let parameters = [NamedParameter::required("position", integer_domain())];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let metadata = Rc::new(1);
    let prefix = [("receipt", PureType::Result(metadata.clone()))];
    let environment = Environment {
        panic: Cell::new(true),
        ..Default::default()
    };
    let arguments = [argument("position", field("receipt", 7))];
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_call_arguments(&arguments, &schema, &prefix, &environment, LIMITS)
    }));
    assert!(unwind.is_err());
    assert_eq!(Rc::strong_count(&metadata), 2);
    environment.panic.set(false);
    assert!(check_call_arguments(&arguments, &schema, &prefix, &environment, LIMITS).is_ok());
    assert_eq!(Rc::strong_count(&metadata), 2);
}

#[test]
fn another_native_schema_supports_borrowed_nonclone_nonserde_gui_local_metadata() {
    #[derive(PartialEq)]
    enum Operation {
        Caption,
    }
    enum Field {
        Caption,
    }
    struct Metadata(Rc<()>);
    impl PartialEq for Metadata {
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self, other)
        }
    }
    struct NativeEnvironment<'a>(&'a Metadata);
    impl<'a> PureTypeEnvironment<Field, Operation> for NativeEnvironment<'a> {
        type Result = &'a Metadata;
        fn field_type(&self, result: &&Metadata, _: &Field) -> Option<ScalarType> {
            std::ptr::eq(*result, self.0).then_some(ScalarType::String)
        }
        fn member_result(&self, _: &&Metadata, _: &str, _: &Operation) -> Option<&'a Metadata> {
            None
        }
    }
    type GuiIr = Computation<Field, Operation, Metadata, Metadata>;
    let metadata = Metadata(Rc::new(()));
    let parameters = [NamedParameter::required(
        "caption",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schemas = [OperationSchema {
        key: Operation::Caption,
        parameters: &parameters,
        result: Metadata(Rc::new(())),
        required_capability: "view.edit",
    }];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let expression = GuiIr::Call {
        operation: Operation::Caption,
        arguments: vec![ComputedArgument {
            name: "caption".into(),
            value: GuiIr::Field {
                value: Box::new(GuiIr::Local {
                    name: "view".into(),
                }),
                field: Field::Caption,
            },
        }],
    };
    let environment = NativeEnvironment(&metadata);
    let host = CallTypeHost {
        catalog: &catalog,
        version: 7,
        granted: &["view.edit"],
        environment: &environment,
    };
    let result = infer_call_type(
        &expression,
        &[("view", PureType::Result(&metadata))],
        &host,
        LIMITS,
    )
    .unwrap();
    assert!(std::ptr::eq(result, &schemas[0].result));
    assert_eq!(Rc::strong_count(&metadata.0), 1);
    assert_eq!(Rc::strong_count(&result.0), 1);
}

#[test]
fn duplicate_schema_keys_and_argument_scopes_do_not_escape_preflight_or_parameters() {
    let schema_parameters = [
        NamedParameter::required("a", integer_domain()),
        NamedParameter::required("a", integer_domain()),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &schema_parameters,
        result: (),
        required_capability: (),
    };
    assert_eq!(
        check_call_arguments(
            &[argument("a", number(1))],
            &schema,
            &[],
            &Environment::default(),
            LIMITS
        ),
        Err(CallTypeError::Names(
            NamedArgumentError::DuplicateParameter { index: 1 }
        ))
    );

    let parameters = [
        NamedParameter::required("first", integer_domain()),
        NamedParameter::required("last", integer_domain()),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let first = Ir::Bind {
        name: "temporary".into(),
        value: Box::new(number(1)),
        body: Box::new(local("temporary")),
    };
    let arguments = [
        argument("last", local("temporary")),
        argument("first", first),
    ];
    assert_eq!(
        check_call_arguments(&arguments, &schema, &[], &Environment::default(), LIMITS),
        Err(CallTypeError::Inference {
            argument_index: 0,
            error: PureTypeError::UnknownLocal
        })
    );
}

#[test]
fn parameterless_calls_do_not_clone_unused_type_metadata() {
    struct Metadata;
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            panic!("unused native metadata clone")
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, _: &Self) -> bool {
            panic!("unused native metadata equality")
        }
    }
    struct Host;
    impl PureTypeEnvironment<u8, u32> for Host {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &u8) -> Option<ScalarType> {
            panic!("unused native field")
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &u32) -> Option<Metadata> {
            panic!("unused native member")
        }
    }
    let schema: OperationSchema<'_, (), &str, ScalarTypeSet, (), ()> = OperationSchema {
        key: (),
        parameters: &[],
        result: (),
        required_capability: (),
    };
    let arguments: [ComputedArgument<Ir>; 0] = [];
    assert!(
        check_call_arguments(
            &arguments,
            &schema,
            &[("unused", PureType::Result(Metadata))],
            &Host,
            LIMITS
        )
        .is_ok()
    );
}

#[test]
fn static_errors_and_host_debug_never_format_private_data() {
    let parameters = [NamedParameter::required("position", integer_domain())];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: (),
        required_capability: 31,
    }];
    let catalog = OperationCatalog::new(
        9,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &Environment::default(),
    };
    let error = infer_call_type(
        &call(vec![argument("position", text("private-secret"))]),
        &[],
        &host,
        LIMITS,
    )
    .unwrap_err();
    let diagnostics = format!("{error:?} {error} {host:?}");
    for payload in ["private-secret", "position", "31", "17"] {
        assert!(!diagnostics.contains(payload));
    }
}
