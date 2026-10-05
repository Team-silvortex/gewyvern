use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{CallEvaluationLimits, evaluate_call_arguments_in_scope};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits};
use leselang_hir::source_call::*;
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, parse};

const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 64,
    max_source_depth: 8,
    max_lowered_nodes: 64,
    max_lowered_depth: 8,
    max_arguments: 4,
};
struct Key {
    name: String,
    queries: Rc<Cell<usize>>,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl std::borrow::Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.queries.set(self.queries.get() + 1);
        &self.name
    }
}
struct Field(Rc<Cell<usize>>);
impl Drop for Field {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
struct Opcode;
struct NativeEffect;
struct ResultTag;
type Node = Computation<Field, Opcode, NativeEffect, ResultTag>;
struct NativeError(&'static str);
struct Domain {
    events: Rc<RefCell<Vec<&'static str>>>,
    unwind: Rc<Cell<bool>>,
}
impl ScalarArgumentDomain for Domain {
    fn scalar_types(&self) -> ScalarTypeSet {
        self.events.borrow_mut().push("types");
        ScalarTypeSet::only(ScalarType::Integer)
    }
    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        self.events.borrow_mut().push("literal");
        assert!(!self.unwind.get(), "native domain unwind");
        matches!(value, ScalarValue::Integer(value) if *value <= 100)
    }
}
fn expression(source: &str) -> Expression {
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn domain(events: &Rc<RefCell<Vec<&'static str>>>, unwind: &Rc<Cell<bool>>) -> Domain {
    Domain {
        events: events.clone(),
        unwind: unwind.clone(),
    }
}
fn catalog<'a>(
    schema: &'a [SourceSchema<'a, Key, Domain, ResultTag, u8>],
) -> OperationCatalog<'a, Key, &'a str, Domain, ResultTag, u8> {
    OperationCatalog::new(
        7,
        schema,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 4,
        },
    )
    .unwrap()
}
fn operand(argument: &NamedArgument) -> Result<(Node, Option<ScalarType>), NativeError> {
    leselang_hir::scalar_source::lower_scalar_source(&argument.value, LIMITS, |_| {
        Err(NativeError("private unsupported operand"))
    })
    .map(|(value, ty)| (value, Some(ty)))
    .map_err(|_| NativeError("private invalid scalar operand"))
}
struct Environment;
impl PureEvaluationEnvironment<Field, Opcode> for Environment {
    type Result = ();
    type Error = NativeError;
    fn field(&self, _: &(), _: &Field) -> Result<ScalarValue, NativeError> {
        Err(NativeError("no result"))
    }
    fn member(&self, _: &(), _: &str, _: &Opcode) -> Result<(), NativeError> {
        Err(NativeError("no group"))
    }
}

#[test]
fn parsed_native_call_preserves_original_schema_positions_and_prepares_actual_values() {
    let source = expression("fn main() = device.move(b: 6, a: add(left: 1, right: 2))");
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [
        NamedParameter::required("a", domain(&events, &unwind)),
        NamedParameter::required("b", domain(&events, &unwind)),
        NamedParameter::optional("c", domain(&events, &unwind)),
    ];
    let queries = Rc::new(Cell::new(0));
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: queries.clone(),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    let Expression::Call { arguments, .. } = &source else {
        panic!()
    };
    let mut seen = Vec::new();
    let lowered = lower_source_call(&source, &host, LIMITS, |argument| {
        seen.push(argument);
        operand(argument)
    })
    .unwrap();
    assert!(std::ptr::eq(lowered.schema(), &schemas[0]));
    assert!(std::ptr::eq(seen[0], &arguments[1]));
    assert!(std::ptr::eq(seen[1], &arguments[0]));
    assert_eq!(
        lowered
            .arguments()
            .iter()
            .map(|argument| (
                argument.name,
                argument.parameter_index,
                argument.argument_index
            ))
            .collect::<Vec<_>>(),
        [("a", 0, 1), ("b", 1, 0)]
    );
    assert_eq!(format!("{lowered:?}"), "LoweredSourceCall { arguments: 2 }");
    assert_eq!(*events.borrow(), ["types", "types", "literal"]);
    let schema = lowered.schema();
    let arguments = lowered.into_arguments();
    let mut values = Vec::new();
    let mut scope = ScopeFrame::new(&mut values);
    let mut fuel = Fuel::new(100);
    let prepared = evaluate_call_arguments_in_scope(
        &arguments,
        schema,
        &mut scope,
        &Environment,
        &mut fuel,
        CallEvaluationLimits {
            pure: PureEvaluationLimits {
                max_nodes: 64,
                max_depth: 8,
                max_bindings: 4,
            },
            max_arguments: 4,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), schema));
    assert_eq!(
        prepared
            .arguments()
            .iter()
            .map(|argument| (&argument.value, argument.argument_index))
            .collect::<Vec<_>>(),
        [(&ScalarValue::Integer(3), 0), (&ScalarValue::Integer(6), 1)]
    );
    assert_eq!(fuel.remaining(), 96);
}

#[test]
fn version_grants_and_all_names_precede_any_operand_or_domain_callback() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [NamedParameter::required("a", domain(&events, &unwind))];
    let queries = Rc::new(Cell::new(0));
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: queries.clone(),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    for (source, version, grants, reason) in [
        ("device.move(a: missing)", 8, &[3][..], 0),
        ("device.move(a: missing)", 7, &[][..], 1),
        ("device.unknown(a: missing)", 7, &[3][..], 2),
        ("device.move(private: missing)", 7, &[3][..], 3),
        ("device.move()", 7, &[3][..], 4),
        ("device.move(a: missing, a: missing)", 7, &[3][..], 5),
    ] {
        let source = expression(&format!("fn main() = {source}"));
        queries.set(0);
        let host = SourceCallHost {
            catalog: &catalog,
            version,
            granted: grants,
        };
        let error = lower_source_call(
            &source,
            &host,
            LIMITS,
            |_: &NamedArgument| -> Result<(Node, Option<ScalarType>), NativeError> {
                panic!("operand must not run")
            },
        )
        .unwrap_err();
        match reason {
            0 => {
                assert!(matches!(
                    error,
                    SourceCallError::Catalog(OperationCatalogError::UnsupportedVersion)
                ));
                assert_eq!(queries.get(), 0);
            }
            1 => assert!(matches!(
                error,
                SourceCallError::Catalog(OperationCatalogError::CapabilityDenied)
            )),
            2 => assert!(matches!(
                error,
                SourceCallError::Catalog(OperationCatalogError::UnknownOperation)
            )),
            3 => assert!(matches!(
                error,
                SourceCallError::Names(NamedArgumentError::UnknownArgument { index: 0 })
            )),
            4 => assert!(matches!(
                error,
                SourceCallError::Names(NamedArgumentError::MissingArgument { parameter_index: 0 })
            )),
            _ => assert!(matches!(error, SourceCallError::ArgumentLimit)),
        }
        assert!(events.borrow().is_empty());
        assert!(!format!("{error:?}").contains("private"));
    }
}

#[test]
fn late_cold_ast_failure_precedes_even_native_catalog_lookup() {
    let mut source =
        expression("fn main() = device.move(a: 1, b: choose(when: true, then: 2, otherwise: 3))");
    let Expression::Call { arguments, .. } = &mut source else {
        panic!()
    };
    let Expression::Call {
        arguments: cold, ..
    } = &mut arguments[1].value
    else {
        panic!()
    };
    let rejected_span = cold[2].span;
    cold[2].value = Expression::String {
        value: "private".repeat(1024),
        span: rejected_span,
    };
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [
        NamedParameter::required("a", domain(&events, &unwind)),
        NamedParameter::required("b", domain(&events, &unwind)),
    ];
    let queries = Rc::new(Cell::new(0));
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: queries.clone(),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    let error = lower_source_call(&source, &host, LIMITS, operand).unwrap_err();
    assert!(
        matches!(error, SourceCallError::Shape { span, error: SourceShapeError::UnboundedText } if span == rejected_span)
    );
    assert_eq!(queries.get(), 0);
    assert!(events.borrow().is_empty());
}

#[test]
fn source_and_generated_forests_have_separate_aggregate_budgets() {
    let source = expression("fn main() = device.move(a: 1, b: 2)");
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [
        NamedParameter::required("a", domain(&events, &unwind)),
        NamedParameter::required("b", domain(&events, &unwind)),
    ];
    let queries = Rc::new(Cell::new(0));
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: queries.clone(),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    for limits in [
        SourceCallLimits {
            max_source_nodes: 2,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 0,
            ..LIMITS
        },
        SourceCallLimits {
            max_arguments: 65,
            ..LIMITS
        },
    ] {
        assert!(lower_source_call(&source, &host, limits, operand).is_err());
        assert_eq!(queries.get(), 0);
        assert!(events.borrow().is_empty());
    }
    let calls = Cell::new(0);
    let error = lower_source_call(
        &source,
        &host,
        SourceCallLimits {
            max_lowered_nodes: 2,
            ..LIMITS
        },
        |argument| {
            calls.set(calls.get() + 1);
            operand(argument)
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SourceCallError::Output {
            argument_index: 1,
            error: leselang_hir::pure_typing::PureTypeError::Structure(StructureError::NodeLimit)
        }
    ));
    assert_eq!(*events.borrow(), ["types", "literal"]);
    assert_eq!(calls.get(), 1);
}

#[test]
fn incorrect_or_impure_native_output_is_rejected_without_native_literal_queries() {
    let source = expression("fn main() = device.move(a: 1)");
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [NamedParameter::required("a", domain(&events, &unwind))];
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: Rc::new(Cell::new(0)),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    for (value, ty, expected) in [
        (
            Node::Literal {
                value: ScalarValue::Boolean(true),
            },
            Some(ScalarType::Integer),
            ArgumentTypeError::InconsistentLiteralType,
        ),
        (
            Node::Literal {
                value: ScalarValue::Integer(1),
            },
            None,
            ArgumentTypeError::NonScalar,
        ),
        (
            Node::Host {
                effect: Box::new(NativeEffect),
            },
            Some(ScalarType::Integer),
            ArgumentTypeError::Impure,
        ),
    ] {
        events.borrow_mut().clear();
        let mut value = Some(value);
        let error = lower_source_call(&source, &host, LIMITS, |_| {
            Ok::<_, NativeError>((value.take().unwrap(), ty))
        })
        .unwrap_err();
        assert!(
            matches!(error, SourceCallError::Argument { parameter_index: 0, argument_index: 0, error } if error == expected)
        );
        assert!(!events.borrow().contains(&"literal"));
    }
}

#[test]
fn partial_native_output_is_released_on_callback_failure_or_unwind() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [
        NamedParameter::required("a", domain(&events, &unwind)),
        NamedParameter::required("b", domain(&events, &unwind)),
    ];
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: Rc::new(Cell::new(0)),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    for panic in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            lower_source_call(&source, &host, LIMITS, |argument| {
                if argument.name == "b" {
                    assert!(!panic, "native lowering unwind");
                    return Err(NativeError("private lowering failure"));
                }
                Ok((
                    Node::Field {
                        value: Box::new(Node::Local { name: "r".into() }),
                        field: Field(drops.clone()),
                    },
                    Some(ScalarType::Integer),
                ))
            })
        }));
        if panic {
            assert!(outcome.is_err());
        } else {
            let error = outcome.unwrap().unwrap_err();
            assert!(!format!("{error:?}").contains("private"));
            let SourceCallError::Lowering {
                argument_index,
                error: NativeError(message),
            } = error
            else {
                panic!()
            };
            assert_eq!(argument_index, 0);
            assert_eq!(message, "private lowering failure");
        }
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn native_domain_unwind_is_not_converted_to_a_fault_or_automatically_retried() {
    let source = expression("fn main() = device.move(a: 1)");
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(true));
    let parameters = [NamedParameter::required("a", domain(&events, &unwind))];
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: Rc::new(Cell::new(0)),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| lower_source_call(
            &source, &host, LIMITS, operand
        )))
        .is_err()
    );
    assert_eq!(*events.borrow(), ["types", "literal"]);
}

#[test]
fn duplicate_names_parameter_ceiling_and_zero_generated_budget_precede_lowering() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [
        NamedParameter::required("a", domain(&events, &unwind)),
        NamedParameter::optional("b", domain(&events, &unwind)),
    ];
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: Rc::new(Cell::new(0)),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    let duplicate = expression("fn main() = device.move(a: missing, a: missing)");
    let error = lower_source_call(&duplicate, &host, LIMITS, operand).unwrap_err();
    assert!(matches!(
        error,
        SourceCallError::Names(NamedArgumentError::DuplicateArgument { index: 1 })
    ));
    let source = expression("fn main() = device.move(a: 1)");
    let never = |_: &NamedArgument| -> Result<(Node, Option<ScalarType>), NativeError> {
        panic!("lowering must not run")
    };
    let error = lower_source_call(
        &source,
        &host,
        SourceCallLimits {
            max_arguments: 1,
            ..LIMITS
        },
        never,
    )
    .unwrap_err();
    assert!(matches!(error, SourceCallError::ParameterLimit));
    let error = lower_source_call(
        &source,
        &host,
        SourceCallLimits {
            max_lowered_nodes: 0,
            ..LIMITS
        },
        never,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SourceCallError::OutputRoot(StructureError::NodeLimit)
    ));
    assert!(events.borrow().is_empty());
}

#[test]
fn forged_names_and_noncall_nodes_never_query_native_keys() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let unwind = Rc::new(Cell::new(false));
    let parameters = [NamedParameter::required("a", domain(&events, &unwind))];
    let queries = Rc::new(Cell::new(0));
    let schemas = [SourceSchema {
        key: Key {
            name: "device.move".into(),
            queries: queries.clone(),
        },
        parameters: &parameters,
        result: ResultTag,
        required_capability: 3,
    }];
    let catalog = catalog(&schemas);
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[3],
    };
    let source = expression("fn main() = 1");
    assert!(matches!(
        lower_source_call(&source, &host, LIMITS, operand),
        Err(SourceCallError::NotCall)
    ));
    for invalid in ["private..move", "private".repeat(200).as_str()] {
        let mut source = expression("fn main() = device.move(a: 1)");
        let Expression::Call { callee, .. } = &mut source else {
            panic!()
        };
        *callee = invalid.to_owned();
        assert!(matches!(
            lower_source_call(&source, &host, LIMITS, operand),
            Err(SourceCallError::Shape {
                error: SourceShapeError::InvalidName,
                ..
            })
        ));
    }
    assert_eq!(queries.get(), 0);
    assert!(events.borrow().is_empty());
}

#[test]
fn unrelated_text_schema_distinguishes_omission_from_explicit_none_without_copying_buffers() {
    let parameters = [
        NamedParameter::required("caption", ScalarTypeSet::only(ScalarType::String)),
        NamedParameter::optional("hint", ScalarTypeSet::only(ScalarType::None)),
    ];
    let schemas = [SourceSchema {
        key: String::from("panel.caption"),
        parameters: &parameters,
        result: (),
        required_capability: "panel",
    }];
    let catalog = OperationCatalog::new(
        2,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 2,
        },
    )
    .unwrap();
    let host = SourceCallHost {
        catalog: &catalog,
        version: 2,
        granted: &["panel"],
    };
    for (args, expected) in [
        ("caption: \"ready\"", 1),
        ("hint: none, caption: \"ready\"", 2),
    ] {
        let source = expression(&format!("fn main() = panel.caption({args})"));
        let mut buffer = None;
        let lowered = lower_source_call(&source, &host, LIMITS, |argument| {
            let value = match &argument.value {
                Expression::String { value, .. } => {
                    let value = value.clone();
                    buffer = Some(value.as_ptr());
                    ScalarValue::String(value)
                }
                Expression::None { .. } => ScalarValue::None,
                _ => panic!(),
            };
            let ty = value.scalar_type();
            Ok::<_, NativeError>((Node::Literal { value }, Some(ty)))
        })
        .unwrap();
        assert_eq!(lowered.arguments().len(), expected);
        assert!(std::ptr::eq(lowered.schema(), &schemas[0]));
        let arguments = lowered.into_arguments();
        let Node::Literal {
            value: ScalarValue::String(value),
        } = &arguments[0].value
        else {
            panic!()
        };
        assert_eq!(value.as_ptr(), buffer.unwrap());
        if expected == 2 {
            assert_eq!(arguments[1].name, "hint");
            assert!(matches!(
                arguments[1].value,
                Node::Literal {
                    value: ScalarValue::None
                }
            ));
        }
    }
}
