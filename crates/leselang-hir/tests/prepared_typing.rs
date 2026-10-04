use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leselang_hir::call_typing::{CallTypeError, CallTypeHost, CallTypeLimits};
use leselang_hir::ir::{Computation, ComputedArgument, GroupKind};
use leselang_hir::prepared_typing::{
    PreparedCallSchema, PreparedCallSchemas, PreparedCallTypeError, infer_prepared_call_type,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
};
use leselang_runtime_core::{
    ArgumentTypeError, BinaryOperator, NamedArgumentError, NamedParameter, OperationCatalog,
    OperationCatalogError, OperationCatalogLimits, OperationSchema, ScalarArgumentDomain,
    ScalarType, ScalarTypeSet, ScalarValue,
};

type Ir = Computation<u8, u32, Rc<()>, ()>;
const LIMITS: CallTypeLimits = CallTypeLimits {
    pure: TypeInferenceLimits {
        max_nodes: 1024,
        max_depth: 16,
        max_bindings: 16,
    },
    max_arguments: 64,
};

#[derive(Default)]
struct Environment {
    fields: Cell<usize>,
    panic: Cell<bool>,
}
impl PureTypeEnvironment<u8, u32> for Environment {
    type Result = Rc<u8>;
    fn field_type(&self, result: &Rc<u8>, field: &u8) -> Option<ScalarType> {
        self.fields.set(self.fields.get() + 1);
        assert!(!self.panic.get(), "private native query");
        (**result == 1 && *field == 7).then_some(ScalarType::Integer)
    }
    fn member_result(&self, group: &Rc<u8>, name: &str, operation: &u32) -> Option<Rc<u8>> {
        (**group == 2 && name == "move" && *operation == 17).then(|| Rc::new(1))
    }
}
fn literal(value: ScalarValue) -> Ir {
    Ir::Literal { value }
}
fn number(value: u64) -> Ir {
    literal(ScalarValue::Integer(value))
}
fn boolean(value: bool) -> Ir {
    literal(ScalarValue::Boolean(value))
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(value: Ir) -> Ir {
    Ir::Field {
        value: Box::new(value),
        field: 7,
    }
}
fn argument(name: &str, value: Ir) -> ComputedArgument<Ir> {
    ComputedArgument {
        name: name.into(),
        value,
    }
}
fn call(operation: u32, value: Ir) -> Ir {
    Ir::Call {
        operation,
        arguments: vec![argument("position", value)],
    }
}
fn bind(name: &str, value: Ir, body: Ir) -> Ir {
    Ir::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn choose(when: Ir, then: Ir, otherwise: Ir) -> Ir {
    Ir::Choose {
        when: Box::new(when),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn catalog<'a>(
    schemas: &'a [OperationSchema<'a, u32, &'a str, ScalarTypeSet, u8, u8>],
) -> OperationCatalog<'a, u32, &'a str, ScalarTypeSet, u8, u8> {
    OperationCatalog::new(
        9,
        schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap()
}

#[test]
fn native_preparation_infers_both_cold_paths_and_returns_original_borrowed_result() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let prefix = [("receipt", PureType::Result(Rc::new(1)))];
    let expression = bind(
        "n",
        field(local("receipt")),
        choose(
            boolean(true),
            call(
                17,
                Ir::Binary {
                    operator: BinaryOperator::Add,
                    left: Box::new(local("n")),
                    right: Box::new(number(1)),
                },
            ),
            call(17, field(local("receipt"))),
        ),
    );
    let result =
        infer_prepared_call_type(&expression, &prefix, &environment, &host, LIMITS).unwrap();
    assert!(std::ptr::eq(result, &schemas[0].result));
    assert_eq!(environment.fields.get(), 2);
    assert!(!expression.is_pure());
}

#[test]
fn different_operations_do_not_join_even_when_declared_result_tags_match() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1,
            required_capability: 31,
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 1,
            required_capability: 31,
        },
    ];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let expression = choose(
        boolean(true),
        call(17, field(local("receipt"))),
        call(18, number(1)),
    );
    assert_eq!(
        infer_prepared_call_type(
            &expression,
            &[("receipt", PureType::Result(Rc::new(1)))],
            &environment,
            &host,
            LIMITS
        ),
        Err(PreparedCallTypeError::InconsistentOperation)
    );
    assert_eq!(environment.fields.get(), 0);
}

#[test]
fn every_cold_schema_version_grant_and_named_shape_precedes_type_queries() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1,
            required_capability: 31,
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 1,
            required_capability: 32,
        },
    ];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let prefix = [("receipt", PureType::Result(Rc::new(1)))];
    let first = || call(17, field(local("receipt")));
    for (other, version, expected) in [
        (
            call(17, number(1)),
            8,
            PreparedCallTypeError::Call {
                call_index: 0,
                error: CallTypeError::Catalog(OperationCatalogError::UnsupportedVersion),
            },
        ),
        (
            call(18, number(1)),
            9,
            PreparedCallTypeError::Call {
                call_index: 1,
                error: CallTypeError::Catalog(OperationCatalogError::CapabilityDenied),
            },
        ),
        (
            call(99, number(1)),
            9,
            PreparedCallTypeError::Call {
                call_index: 1,
                error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation),
            },
        ),
        (
            Ir::Call {
                operation: 17,
                arguments: vec![],
            },
            9,
            PreparedCallTypeError::Call {
                call_index: 1,
                error: CallTypeError::Names(NamedArgumentError::MissingArgument {
                    parameter_index: 0,
                }),
            },
        ),
    ] {
        let host = CallTypeHost {
            catalog: &catalog,
            version,
            granted: &[31],
            environment: &environment,
        };
        assert_eq!(
            infer_prepared_call_type(
                &choose(boolean(true), first(), other),
                &prefix,
                &environment,
                &host,
                LIMITS
            ),
            Err(expected)
        );
    }
    assert_eq!(environment.fields.get(), 0);
}

#[test]
fn invalid_entire_tree_precedes_any_selector_callback_or_metadata_clone() {
    struct Metadata;
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            panic!("private clone")
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, _: &Self) -> bool {
            panic!("private equality")
        }
    }
    struct Host;
    impl PureTypeEnvironment<u8, u32> for Host {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &u8) -> Option<ScalarType> {
            panic!("private field")
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &u32) -> Option<Metadata> {
            panic!("private member")
        }
    }
    impl<'a> PreparedCallSchemas<'a, u32> for Host {
        type Key = u32;
        type Domain = ScalarTypeSet;
        type Result = ();
        type Capability = ();
        fn select(
            &self,
            _: &u32,
        ) -> Result<&'a PreparedCallSchema<'a, u32, ScalarTypeSet, (), ()>, OperationCatalogError>
        {
            panic!("private selector")
        }
    }
    let expression = choose(
        boolean(true),
        call(17, local("receipt")),
        call(
            17,
            choose(
                boolean(true),
                number(1),
                Ir::Host {
                    effect: Box::new(Rc::new(())),
                },
            ),
        ),
    );
    assert_eq!(
        infer_prepared_call_type(
            &expression,
            &[("receipt", PureType::Result(Metadata))],
            &Host,
            &Host,
            LIMITS
        ),
        Err(PreparedCallTypeError::Call {
            call_index: 1,
            error: CallTypeError::Inference {
                argument_index: 0,
                error: PureTypeError::Impure
            }
        })
    );
}

#[test]
fn wrappers_and_all_cold_children_share_one_physical_node_and_depth_budget() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let expression = bind(
        "n",
        number(1),
        choose(boolean(true), call(17, local("n")), call(17, number(2))),
    );
    let limits = CallTypeLimits {
        pure: TypeInferenceLimits {
            max_nodes: 8,
            max_depth: 3,
            max_bindings: 1,
        },
        max_arguments: 1,
    };
    assert!(infer_prepared_call_type(&expression, &[], &environment, &host, limits).is_ok());
    assert!(
        infer_prepared_call_type(
            &expression,
            &[],
            &environment,
            &host,
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 7,
                    ..limits.pure
                },
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        infer_prepared_call_type(
            &expression,
            &[],
            &environment,
            &host,
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_depth: 2,
                    ..limits.pure
                },
                ..limits
            }
        )
        .is_err()
    );
    assert_eq!(
        infer_prepared_call_type(
            &expression,
            &[],
            &environment,
            &host,
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_bindings: 0,
                    ..limits.pure
                },
                ..limits
            }
        ),
        Err(PreparedCallTypeError::Preparation(
            PureTypeError::BindingLimit
        ))
    );
}

#[test]
fn lexical_siblings_reuse_names_without_leaks_and_active_names_cannot_shadow() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let sibling = choose(
        boolean(false),
        bind("n", number(1), call(17, local("n"))),
        bind("n", number(2), call(17, local("n"))),
    );
    assert!(infer_prepared_call_type(&sibling, &[], &environment, &host, LIMITS).is_ok());
    let leaking = choose(
        boolean(true),
        bind("n", number(1), call(17, local("n"))),
        call(17, local("n")),
    );
    assert_eq!(
        infer_prepared_call_type(&leaking, &[], &environment, &host, LIMITS),
        Err(PreparedCallTypeError::Call {
            call_index: 1,
            error: CallTypeError::Inference {
                argument_index: 0,
                error: PureTypeError::UnknownLocal
            }
        })
    );
    assert_eq!(
        infer_prepared_call_type(
            &sibling,
            &[("n", PureType::Scalar(ScalarType::Integer))],
            &environment,
            &host,
            LIMITS
        ),
        Err(PreparedCallTypeError::Preparation(
            PureTypeError::ShadowedBinding
        ))
    );
}

#[test]
fn pure_result_and_closed_group_aliases_are_preparation_not_captures() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let receipt = bind("alias", local("receipt"), call(17, field(local("alias"))));
    assert!(
        infer_prepared_call_type(
            &receipt,
            &[("receipt", PureType::Result(Rc::new(1)))],
            &environment,
            &host,
            LIMITS
        )
        .is_ok()
    );
    let member = Ir::Member {
        group: "alias".into(),
        name: "move".into(),
        operation: 17,
    };
    let group = bind("alias", local("group"), call(17, field(member)));
    assert!(
        infer_prepared_call_type(
            &group,
            &[("group", PureType::Result(Rc::new(2)))],
            &environment,
            &host,
            LIMITS
        )
        .is_ok()
    );
}

#[test]
fn cold_guard_and_argument_types_are_checked_without_running_arithmetic() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    assert_eq!(
        infer_prepared_call_type(
            &choose(number(1), call(17, number(1)), call(17, number(2))),
            &[],
            &environment,
            &host,
            LIMITS
        ),
        Err(PreparedCallTypeError::ConditionType)
    );
    let invalid = choose(
        boolean(true),
        call(17, number(1)),
        call(17, literal(ScalarValue::String("private".into()))),
    );
    assert_eq!(
        infer_prepared_call_type(&invalid, &[], &environment, &host, LIMITS),
        Err(PreparedCallTypeError::Call {
            call_index: 1,
            error: CallTypeError::Argument {
                parameter_index: 0,
                argument_index: 0,
                error: ArgumentTypeError::TypeMismatch
            }
        })
    );
    let division = Ir::Binary {
        operator: BinaryOperator::Div,
        left: Box::new(number(1)),
        right: Box::new(number(0)),
    };
    assert!(
        infer_prepared_call_type(
            &bind("n", division, call(17, local("n"))),
            &[],
            &environment,
            &host,
            LIMITS
        )
        .is_ok()
    );
}

#[test]
fn capture_group_host_multi_effect_and_scalar_exit_flows_remain_rejected() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
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
        choose(boolean(true), call(17, number(1)), number(1)),
    ] {
        assert_eq!(
            infer_prepared_call_type(&expression, &[], &environment, &host, LIMITS),
            Err(PreparedCallTypeError::InvalidFlow)
        );
    }
    for expression in [
        bind(
            "receipt",
            call(17, number(1)),
            call(17, field(local("receipt"))),
        ),
        choose(
            Ir::Host {
                effect: Box::new(Rc::new(())),
            },
            call(17, number(1)),
            call(17, number(1)),
        ),
    ] {
        assert_eq!(
            infer_prepared_call_type(&expression, &[], &environment, &host, LIMITS),
            Err(PreparedCallTypeError::Preparation(PureTypeError::Impure))
        );
    }
}

#[test]
fn one_owned_prefix_is_reused_without_cloning_metadata_per_call_argument() {
    struct Metadata<'a> {
        clones: &'a Cell<usize>,
    }
    impl Clone for Metadata<'_> {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                clones: self.clones,
            }
        }
    }
    impl PartialEq for Metadata<'_> {
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self.clones, other.clones)
        }
    }
    impl<'a> PureTypeEnvironment<u8, u32> for Metadata<'a> {
        type Result = Metadata<'a>;
        fn field_type(&self, _: &Self::Result, _: &u8) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &Self::Result, _: &str, _: &u32) -> Option<Self::Result> {
            None
        }
    }
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let clones = Cell::new(0);
    let environment = Metadata { clones: &clones };
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let expression = choose(boolean(true), call(17, number(1)), call(17, number(2)));
    assert!(
        infer_prepared_call_type(
            &expression,
            &[("unused", PureType::Result(Metadata { clones: &clones }))],
            &environment,
            &host,
            LIMITS
        )
        .is_ok()
    );
    assert_eq!(clones.get(), 1);
}

#[test]
fn query_unwind_drops_preparation_metadata_and_preserves_external_scope() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment {
        panic: Cell::new(true),
        ..Default::default()
    };
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let metadata = Rc::new(1);
    let prefix = [("receipt", PureType::Result(metadata.clone()))];
    let expression = bind("alias", local("receipt"), call(17, field(local("alias"))));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| infer_prepared_call_type(
            &expression,
            &prefix,
            &environment,
            &host,
            LIMITS
        )))
        .is_err()
    );
    assert_eq!(Rc::strong_count(&metadata), 2);
    environment.panic.set(false);
    assert!(infer_prepared_call_type(&expression, &prefix, &environment, &host, LIMITS).is_ok());
    assert_eq!(Rc::strong_count(&metadata), 2);
}

#[test]
fn a_distinct_gui_schema_uses_nonclone_native_slots_and_borrowed_result_payloads() {
    #[derive(PartialEq)]
    enum Operation {
        Caption,
    }
    struct Field;
    struct Native(Rc<()>);
    struct Host;
    impl PureTypeEnvironment<Field, Operation> for Host {
        type Result = ();
        fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
            None
        }
    }
    type GuiIr = Computation<Field, Operation, Native, Native>;
    let parameters = [NamedParameter::required(
        "caption",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schemas = [OperationSchema {
        key: Operation::Caption,
        parameters: &parameters,
        result: Native(Rc::new(())),
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
    let environment = Host;
    let host = CallTypeHost {
        catalog: &catalog,
        version: 7,
        granted: &["view.edit"],
        environment: &environment,
    };
    let terminal = || GuiIr::Call {
        operation: Operation::Caption,
        arguments: vec![ComputedArgument {
            name: "caption".into(),
            value: GuiIr::Local {
                name: "label".into(),
            },
        }],
    };
    let expression = GuiIr::Bind {
        name: "label".into(),
        value: Box::new(GuiIr::Literal {
            value: ScalarValue::String("ready".into()),
        }),
        body: Box::new(GuiIr::Choose {
            when: Box::new(GuiIr::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(terminal()),
            otherwise: Box::new(terminal()),
        }),
    };
    let result = infer_prepared_call_type(&expression, &[], &environment, &host, LIMITS).unwrap();
    assert!(std::ptr::eq(result, &schemas[0].result));
    assert_eq!(Rc::strong_count(&result.0), 1);
}

#[test]
fn error_observations_do_not_echo_native_or_source_payloads() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let error = infer_prepared_call_type(
        &bind(
            "private_name",
            number(1),
            call(17, literal(ScalarValue::String("private-secret".into()))),
        ),
        &[],
        &environment,
        &host,
        LIMITS,
    )
    .unwrap_err();
    let observation = format!("{error:?} {error}");
    for private in ["private_name", "private-secret", "position", "17", "31"] {
        assert!(!observation.contains(private));
    }
}

#[test]
fn cold_catalog_rejection_precedes_even_unused_prefix_metadata_cloning() {
    struct Metadata;
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            panic!("private unused clone")
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, _: &Self) -> bool {
            panic!("private unused equality")
        }
    }
    struct Host;
    impl PureTypeEnvironment<u8, u32> for Host {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &u8) -> Option<ScalarType> {
            panic!("private native field")
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &u32) -> Option<Metadata> {
            panic!("private native member")
        }
    }
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1,
        required_capability: 31,
    }];
    let catalog = catalog(&schemas);
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &Host,
    };
    let expression = choose(boolean(true), call(17, number(1)), call(99, number(2)));
    assert_eq!(
        infer_prepared_call_type(
            &expression,
            &[("unused", PureType::Result(Metadata))],
            &Host,
            &host,
            LIMITS
        ),
        Err(PreparedCallTypeError::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation)
        })
    );
}

#[test]
fn native_literal_domains_check_all_cold_calls_in_declaration_order() {
    struct Domain<'a>(&'a RefCell<Vec<&'static str>>, &'static str);
    impl ScalarArgumentDomain for Domain<'_> {
        fn scalar_types(&self) -> ScalarTypeSet {
            self.0.borrow_mut().push(self.1);
            ScalarTypeSet::only(ScalarType::Integer)
        }
        fn accepts_literal(&self, value: &ScalarValue) -> bool {
            !matches!(value, ScalarValue::Integer(99))
        }
    }
    let events = RefCell::new(vec![]);
    let parameters = [
        NamedParameter::optional("unused", Domain(&events, "unused")),
        NamedParameter::required("first", Domain(&events, "first")),
        NamedParameter::required("last", Domain(&events, "last")),
    ];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: (),
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(
        9,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap();
    let environment = Environment::default();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[31],
        environment: &environment,
    };
    let terminal = |last| Ir::Call {
        operation: 17,
        arguments: vec![argument("last", number(last)), argument("first", number(1))],
    };
    let error = infer_prepared_call_type(
        &choose(boolean(true), terminal(2), terminal(99)),
        &[],
        &environment,
        &host,
        LIMITS,
    )
    .unwrap_err();
    assert_eq!(
        error,
        PreparedCallTypeError::Call {
            call_index: 1,
            error: CallTypeError::Argument {
                parameter_index: 2,
                argument_index: 0,
                error: ArgumentTypeError::InvalidLiteralDomain
            }
        }
    );
    assert_eq!(*events.borrow(), ["first", "last", "first", "last"]);
}
