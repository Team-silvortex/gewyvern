use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leselang_hir::call_typing::{CallTypeError, CallTypeHost, CallTypeLimits};
use leselang_hir::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError as Error, CallFlowTypeLimits, infer_call_flow_type,
};
use leselang_hir::ir::{Computation, ComputedArgument, GroupKind};
use leselang_hir::prepared_typing::{PreparedCallSchemas, SelectedPreparedCall};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
};
use leselang_runtime_core::{
    BinaryOperator, NamedArgumentError, NamedParameter, OperationCatalog, OperationCatalogError,
    OperationCatalogLimits, OperationSchema, ScalarType, ScalarTypeSet, ScalarValue,
};

type Ir = Computation<u8, u32, Rc<()>, ()>;
const LIMITS: CallFlowTypeLimits = CallFlowTypeLimits {
    call: CallTypeLimits {
        pure: TypeInferenceLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
        },
        max_arguments: 3,
    },
    max_calls: 16,
};
#[derive(Default)]
struct Environment {
    queries: RefCell<Vec<&'static str>>,
    common: bool,
    panic: Cell<bool>,
    missing: bool,
}
impl PureTypeEnvironment<u8, u32> for Environment {
    type Result = u8;
    fn field_type(&self, result: &u8, field: &u8) -> Option<ScalarType> {
        assert!(!self.panic.get(), "private field query");
        self.queries.borrow_mut().push("field");
        ((*field == 7 && matches!(result, 1..=3)) || (*field == 8 && *result == 1))
            .then_some(ScalarType::Integer)
    }
    fn member_result(&self, group: &u8, name: &str, operation: &u32) -> Option<u8> {
        self.queries.borrow_mut().push("member");
        (*group == 9 && name == "move" && *operation == 17).then_some(1)
    }
    fn join_results(&self, left: u8, right: u8) -> Option<u8> {
        self.queries.borrow_mut().push("join");
        if left == right {
            Some(left)
        } else {
            self.common.then_some(3)
        }
    }
}
impl<'schema> CallFlowEnvironment<'schema, u8, u32, u8> for Environment {
    fn call_result_type(&self, _: &u32, declaration: &'schema u8) -> Option<PureType<u8>> {
        assert!(!self.panic.get(), "private result query");
        self.queries.borrow_mut().push("result");
        (!self.missing).then_some(PureType::Result(*declaration))
    }
}
fn number(n: u64) -> Ir {
    Ir::Literal {
        value: ScalarValue::Integer(n),
    }
}
fn boolean(b: bool) -> Ir {
    Ir::Literal {
        value: ScalarValue::Boolean(b),
    }
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(value: Ir, field: u8) -> Ir {
    Ir::Field {
        value: Box::new(value),
        field,
    }
}
fn call(operation: u32, value: Ir) -> Ir {
    Ir::Call {
        operation,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value,
        }],
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
fn check_policy(
    expression: &Ir,
    prefix: &[(&str, PureType<u8>)],
    environment: &Environment,
    limits: CallFlowTypeLimits,
    version: u32,
    grants: &[u8],
) -> Result<PureType<u8>, Error> {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1,
            required_capability: 31,
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 2,
            required_capability: 32,
        },
    ];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version,
        granted: grants,
        environment,
    };
    infer_call_flow_type(expression, prefix, environment, &host, limits)
}
fn check(
    expression: &Ir,
    prefix: &[(&str, PureType<u8>)],
    environment: &Environment,
    limits: CallFlowTypeLimits,
) -> Result<PureType<u8>, Error> {
    check_policy(expression, prefix, environment, limits, 9, &[31, 32])
}

#[test]
fn capture_projection_successor_and_scalar_tail_use_one_declared_type_chain() {
    let environment = Environment::default();
    let expression = bind(
        "first",
        call(17, number(7)),
        bind(
            "second",
            call(18, field(local("first"), 7)),
            Ir::Binary {
                operator: BinaryOperator::Eq,
                left: Box::new(field(local("first"), 7)),
                right: Box::new(field(local("second"), 7)),
            },
        ),
    );
    assert_eq!(
        check(&expression, &[], &environment, LIMITS),
        Ok(PureType::Scalar(ScalarType::Boolean))
    );
    assert_eq!(
        *environment.queries.borrow(),
        ["result", "field", "result", "field", "field"]
    );
    assert!(!expression.is_pure());
}

#[test]
fn compatible_distinct_results_join_only_common_exports_not_atomic_identity() {
    let environment = Environment {
        common: true,
        ..Default::default()
    };
    let value = choose(boolean(true), call(17, number(0)), call(18, number(1)));
    assert_eq!(
        check(&value, &[], &Environment::default(), LIMITS),
        Err(Error::BranchTypes)
    );
    let expression = bind("receipt", value.clone(), field(local("receipt"), 7));
    assert_eq!(
        check(&expression, &[], &environment, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let expression = bind("receipt", value, field(local("receipt"), 8));
    assert_eq!(
        check(&expression, &[], &environment, LIMITS),
        Err(Error::Pure(PureTypeError::FieldNotExported))
    );
    assert_eq!(
        check(
            &choose(boolean(false), call(17, number(0)), boolean(true)),
            &[],
            &environment,
            LIMITS
        ),
        Err(Error::BranchTypes)
    );
}

#[test]
fn scope_is_lexical_forward_results_and_sibling_locals_are_not_exports() {
    let environment = Environment::default();
    let branch = || bind("r", call(17, number(0)), field(local("r"), 7));
    assert_eq!(
        check(
            &choose(boolean(false), branch(), branch()),
            &[],
            &environment,
            LIMITS
        ),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    for expression in [
        bind("r", call(17, field(local("r"), 7)), boolean(true)),
        choose(boolean(true), branch(), field(local("r"), 7)),
        bind(
            "r",
            call(17, number(0)),
            bind("r", call(17, number(0)), boolean(true)),
        ),
    ] {
        assert!(matches!(
            check(&expression, &[], &environment, LIMITS),
            Err(Error::Pure(
                PureTypeError::UnknownLocal | PureTypeError::ShadowedBinding
            )) | Err(Error::Call {
                error: CallTypeError::Inference {
                    error: PureTypeError::UnknownLocal,
                    ..
                },
                ..
            })
        ));
    }
    let expression = bind(
        "r",
        call(17, number(0)),
        bind("alias", local("r"), call(18, field(local("alias"), 7))),
    );
    assert_eq!(
        check(&expression, &[], &environment, LIMITS),
        Ok(PureType::Result(2))
    );
}

#[test]
fn cold_catalog_version_capability_and_names_precede_all_type_queries() {
    let environment = Environment {
        panic: Cell::new(true),
        ..Default::default()
    };
    let prefix = [("prior", PureType::Result(1))];
    let expression = choose(
        boolean(true),
        call(17, field(local("prior"), 7)),
        call(18, number(0)),
    );
    assert_eq!(
        check_policy(&expression, &prefix, &environment, LIMITS, 9, &[31]),
        Err(Error::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::CapabilityDenied)
        })
    );
    assert_eq!(
        check_policy(&expression, &prefix, &environment, LIMITS, 8, &[31, 32]),
        Err(Error::Call {
            call_index: 0,
            error: CallTypeError::Catalog(OperationCatalogError::UnsupportedVersion)
        })
    );
    let expression = choose(
        boolean(true),
        call(17, field(local("prior"), 7)),
        call(99, number(0)),
    );
    assert_eq!(
        check(&expression, &prefix, &environment, LIMITS),
        Err(Error::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation)
        })
    );
    let expression = choose(
        boolean(true),
        call(17, field(local("prior"), 7)),
        Ir::Call {
            operation: 18,
            arguments: vec![],
        },
    );
    assert_eq!(
        check(&expression, &prefix, &environment, LIMITS),
        Err(Error::Call {
            call_index: 1,
            error: CallTypeError::Names(NamedArgumentError::MissingArgument { parameter_index: 0 })
        })
    );
    assert!(environment.queries.borrow().is_empty());
}

#[test]
fn complete_physical_preflight_rejects_hidden_effects_before_native_selection() {
    struct Poison;
    impl PreparedCallSchemas<'static, u32> for Poison {
        type Key = u32;
        type Domain = ScalarTypeSet;
        type Result = u8;
        type Capability = u8;
        fn select(
            &self,
            _: &u32,
        ) -> Result<SelectedPreparedCall<'static, u32, Self>, OperationCatalogError> {
            panic!("selector must remain cold")
        }
    }
    let group = || Ir::Group {
        group_kind: GroupKind::Parallel,
        branches: vec![],
    };
    let host = || Ir::Host {
        effect: Box::new(Rc::new(())),
    };
    let candidates = [
        choose(boolean(true), call(17, number(0)), host()),
        bind("g", group(), boolean(true)),
        choose(call(17, number(0)), boolean(true), boolean(false)),
        call(17, bind("r", call(17, number(0)), number(1))),
        field(call(17, number(0)), 7),
        Ir::Recover {
            value: Box::new(number(0)),
            fallback: Box::new(host()),
        },
        Ir::Loop {
            name: "n".into(),
            initial: Box::new(number(0)),
            condition: Box::new(boolean(false)),
            next: Box::new(call(17, number(0))),
            limit: 0,
        },
        Ir::Fold {
            name: "n".into(),
            item: "s".into(),
            items: Box::new(Ir::Strings { items: vec![] }),
            initial: Box::new(number(0)),
            next: Box::new(call(17, number(0))),
            limit: 0,
        },
        choose(
            boolean(false),
            call(17, number(0)),
            bind("bad-name", number(0), boolean(true)),
        ),
    ];
    let environment = Environment {
        panic: Cell::new(true),
        ..Default::default()
    };
    for expression in candidates {
        assert!(infer_call_flow_type(&expression, &[], &environment, &Poison, LIMITS).is_err());
    }
}

#[test]
fn inclusive_physical_budgets_cover_bindings_guards_arguments_and_cold_calls() {
    let environment = Environment::default();
    let expression = bind(
        "r",
        call(17, number(0)),
        choose(boolean(false), field(local("r"), 7), field(local("r"), 7)),
    );
    let exact = CallFlowTypeLimits {
        call: CallTypeLimits {
            pure: TypeInferenceLimits {
                max_nodes: 9,
                max_depth: 3,
                max_bindings: 1,
            },
            max_arguments: 1,
        },
        max_calls: 1,
    };
    assert!(check(&expression, &[], &environment, exact).is_ok());
    let mut too_small = exact;
    too_small.call.pure.max_nodes = 8;
    assert!(matches!(
        check(&expression, &[], &environment, too_small),
        Err(Error::Structure(_)) | Err(Error::Pure(PureTypeError::Structure(_)))
    ));
    too_small = exact;
    too_small.call.pure.max_depth = 2;
    assert!(check(&expression, &[], &environment, too_small).is_err());
    too_small = exact;
    too_small.call.pure.max_bindings = 0;
    assert_eq!(
        check(&expression, &[], &environment, too_small),
        Err(Error::Pure(PureTypeError::BindingLimit))
    );
    too_small = LIMITS;
    too_small.max_calls = 1;
    assert_eq!(
        check(
            &choose(boolean(true), call(17, number(0)), call(17, number(0))),
            &[],
            &environment,
            too_small
        ),
        Err(Error::CallLimit)
    );
    too_small.max_calls = 0;
    assert!(check(&boolean(true), &[], &environment, too_small).is_ok());
    assert_eq!(
        check(&call(17, number(0)), &[], &environment, too_small),
        Err(Error::CallLimit)
    );
    too_small.max_calls = 16_385;
    assert_eq!(
        check(&boolean(true), &[], &environment, too_small),
        Err(Error::InvalidLimits)
    );
}

#[test]
fn type_errors_keep_cold_call_and_original_argument_positions() {
    let expression = bind(
        "r",
        call(17, number(0)),
        choose(
            boolean(true),
            boolean(false),
            bind("s", call(18, boolean(true)), boolean(false)),
        ),
    );
    assert!(matches!(
        check(&expression, &[], &Environment::default(), LIMITS),
        Err(Error::Call {
            call_index: 1,
            error: CallTypeError::Argument {
                argument_index: 0,
                parameter_index: 0,
                ..
            }
        })
    ));
    assert_eq!(
        check(
            &choose(number(0), boolean(true), boolean(false)),
            &[],
            &Environment::default(),
            LIMITS
        ),
        Err(Error::ConditionType)
    );
    assert_eq!(
        check(
            &call(17, number(0)),
            &[],
            &Environment {
                missing: true,
                ..Default::default()
            },
            LIMITS
        ),
        Err(Error::ResultType { call_index: 0 })
    );
}

#[test]
fn pure_recovery_loops_and_external_closed_group_aliases_remain_type_only() {
    let environment = Environment::default();
    let expression = bind(
        "r",
        call(17, number(0)),
        Ir::Recover {
            value: Box::new(Ir::Binary {
                operator: BinaryOperator::Div,
                left: Box::new(field(local("r"), 7)),
                right: Box::new(number(0)),
            }),
            fallback: Box::new(number(7)),
        },
    );
    assert_eq!(
        check(&expression, &[], &environment, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let expression = bind(
        "alias",
        local("group"),
        bind(
            "r",
            call(
                17,
                field(
                    Ir::Member {
                        group: "alias".into(),
                        name: "move".into(),
                        operation: 17,
                    },
                    7,
                ),
            ),
            field(local("r"), 7),
        ),
    );
    assert!(
        check(
            &expression,
            &[("group", PureType::Result(9))],
            &environment,
            LIMITS
        )
        .is_ok()
    );
    assert!(
        check(
            &expression,
            &[("group", PureType::Result(1))],
            &environment,
            LIMITS
        )
        .is_err()
    );
    let zero = Ir::Loop {
        name: "n".into(),
        initial: Box::new(number(0)),
        condition: Box::new(boolean(false)),
        next: Box::new(boolean(true)),
        limit: 0,
    };
    assert_eq!(
        check(&zero, &[], &environment, LIMITS),
        Err(Error::Pure(PureTypeError::LoopState))
    );
}

#[test]
fn cold_unknown_operation_precedes_poison_prefix_clone_and_result_mapping() {
    struct Metadata;
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            panic!("prefix clone must remain cold")
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, _: &Self) -> bool {
            panic!("metadata comparison must remain cold")
        }
    }
    struct Poison;
    impl PureTypeEnvironment<u8, u32> for Poison {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &u8) -> Option<ScalarType> {
            panic!("field query must remain cold")
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &u32) -> Option<Metadata> {
            panic!("member query must remain cold")
        }
    }
    impl<'schema> CallFlowEnvironment<'schema, u8, u32, u8> for Poison {
        fn call_result_type(&self, _: &u32, _: &'schema u8) -> Option<PureType<Metadata>> {
            panic!("result query must remain cold")
        }
    }
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1u8,
        required_capability: 31,
    }];
    let catalog = OperationCatalog::new(
        9,
        &rows,
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
        environment: &Poison,
    };
    let expression = bind("r", call(17, number(0)), call(99, number(0)));
    assert_eq!(
        infer_call_flow_type(
            &expression,
            &[("unused", PureType::Result(Metadata))],
            &Poison,
            &host,
            LIMITS
        )
        .err(),
        Some(Error::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation)
        })
    );
}

#[test]
fn native_gui_slots_and_result_payloads_are_borrowed_without_clone_debug_or_serde() {
    struct Field;
    #[derive(PartialEq)]
    struct Operation;
    struct HostEffect(Rc<()>);
    struct IrResult;
    #[derive(PartialEq)]
    struct Declaration {
        exports_text: bool,
    }
    struct Gui;
    impl<'schema> PureTypeEnvironment<Field, Operation> for (&'schema Declaration, Gui) {
        type Result = &'schema Declaration;
        fn field_type(&self, result: &&'schema Declaration, _: &Field) -> Option<ScalarType> {
            result.exports_text.then_some(ScalarType::String)
        }
        fn member_result(
            &self,
            _: &&'schema Declaration,
            _: &str,
            _: &Operation,
        ) -> Option<&'schema Declaration> {
            None
        }
    }
    impl<'schema> CallFlowEnvironment<'schema, Field, Operation, Declaration>
        for (&'schema Declaration, Gui)
    {
        fn call_result_type(
            &self,
            _: &Operation,
            declaration: &'schema Declaration,
        ) -> Option<PureType<&'schema Declaration>> {
            Some(PureType::Result(declaration))
        }
    }
    let rows = [OperationSchema {
        key: Operation,
        parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>],
        result: Declaration { exports_text: true },
        required_capability: (),
    }];
    let environment = (&rows[0].result, Gui);
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &environment,
    };
    type GuiIr = Computation<Field, Operation, HostEffect, IrResult>;
    let expression = GuiIr::Bind {
        name: "receipt".into(),
        value: Box::new(GuiIr::Call {
            operation: Operation,
            arguments: vec![],
        }),
        body: Box::new(GuiIr::Local {
            name: "receipt".into(),
        }),
    };
    let result = infer_call_flow_type(&expression, &[], &environment, &host, LIMITS)
        .ok()
        .unwrap();
    let PureType::Result(result) = result else {
        panic!()
    };
    assert!(std::ptr::eq(result, &rows[0].result));
    // These native slots are intentionally neither clonable nor serializable.
    let host_effect = HostEffect(Rc::new(()));
    assert_eq!(Rc::strong_count(&host_effect.0), 1);
}

#[test]
fn query_unwind_releases_captured_metadata_without_mutating_prefix_or_replay() {
    struct Environment {
        receipt: Rc<u8>,
        panic: Cell<bool>,
        maps: Cell<usize>,
    }
    impl PureTypeEnvironment<u8, u32> for Environment {
        type Result = Rc<u8>;
        fn field_type(&self, _: &Rc<u8>, _: &u8) -> Option<ScalarType> {
            assert!(!self.panic.get(), "native type query");
            Some(ScalarType::Integer)
        }
        fn member_result(&self, _: &Rc<u8>, _: &str, _: &u32) -> Option<Rc<u8>> {
            None
        }
    }
    impl<'schema> CallFlowEnvironment<'schema, u8, u32, u8> for Environment {
        fn call_result_type(&self, _: &u32, _: &'schema u8) -> Option<PureType<Rc<u8>>> {
            self.maps.set(self.maps.get() + 1);
            Some(PureType::Result(self.receipt.clone()))
        }
    }
    let environment = Environment {
        receipt: Rc::new(1),
        panic: Cell::new(true),
        maps: Cell::new(0),
    };
    let prefix = [("prior", PureType::Result(environment.receipt.clone()))];
    let rows = [OperationSchema {
        key: 17,
        parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>],
        result: 1u8,
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &environment,
    };
    let expression = bind(
        "r",
        Ir::Call {
            operation: 17,
            arguments: vec![],
        },
        field(local("r"), 7),
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| infer_call_flow_type(
            &expression,
            &prefix,
            &environment,
            &host,
            LIMITS
        )))
        .is_err()
    );
    assert_eq!(Rc::strong_count(&environment.receipt), 2);
    assert_eq!(environment.maps.get(), 1);
    environment.panic.set(false);
    assert!(infer_call_flow_type(&expression, &prefix, &environment, &host, LIMITS).is_ok());
    assert_eq!(Rc::strong_count(&environment.receipt), 2);
    assert_eq!(environment.maps.get(), 2);
}

#[test]
fn failure_messages_and_debug_are_closed_and_payload_free() {
    let error = Error::Call {
        call_index: 7,
        error: CallTypeError::Names(NamedArgumentError::UnknownArgument { index: 3 }),
    };
    assert!(format!("{error:?}").contains("call_index: 7"));
    assert!(!error.to_string().contains("private"));
    assert_eq!(
        Error::ResultType { call_index: 9 }.to_string(),
        "call result declaration has no query type"
    );
}

#[test]
fn unused_prefix_metadata_is_copied_once_across_multiple_captures_and_arguments() {
    struct Metadata {
        id: u8,
        clones: Rc<Cell<usize>>,
    }
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    struct Environment(Rc<Cell<usize>>);
    impl PureTypeEnvironment<u8, u32> for Environment {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &u8) -> Option<ScalarType> {
            Some(ScalarType::Integer)
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &u32) -> Option<Metadata> {
            None
        }
    }
    impl<'schema> CallFlowEnvironment<'schema, u8, u32, u8> for Environment {
        fn call_result_type(
            &self,
            _: &u32,
            declaration: &'schema u8,
        ) -> Option<PureType<Metadata>> {
            Some(PureType::Result(Metadata {
                id: *declaration,
                clones: self.0.clone(),
            }))
        }
    }
    let prefix_clones = Rc::new(Cell::new(0));
    let environment = Environment(Rc::new(Cell::new(0)));
    let prefix = [(
        "unused",
        PureType::Result(Metadata {
            id: 9,
            clones: prefix_clones.clone(),
        }),
    )];
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [OperationSchema {
        key: 17,
        parameters: &parameters,
        result: 1u8,
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &environment,
    };
    let expression = bind(
        "a",
        call(17, number(0)),
        bind(
            "b",
            call(17, field(local("a"), 7)),
            call(17, field(local("b"), 7)),
        ),
    );
    assert!(infer_call_flow_type(&expression, &prefix, &environment, &host, LIMITS).is_ok());
    assert_eq!(prefix_clones.get(), 1);
    assert_eq!(environment.0.get(), 2);
}
