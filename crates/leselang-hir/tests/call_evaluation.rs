use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::*;
use leselang_hir::call_typing::{CallTypeError, CallTypeHost, CallTypeLimits, infer_call_type};
use leselang_hir::ir::{Computation, ComputedArgument};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
};
use leselang_runtime_core::*;

struct Field(&'static str);
#[derive(PartialEq)]
struct Operation(&'static str);
struct OpaqueEffect;
type Ir = Computation<Field, Operation, OpaqueEffect, ()>;

const LIMITS: CallEvaluationLimits = CallEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 4,
};

struct NativeError(String);
struct Record {
    owner: Rc<()>,
    target: String,
    caption: String,
}
struct Editor {
    events: Rc<RefCell<Vec<&'static str>>>,
}
impl PureEvaluationEnvironment<Field, Operation> for Editor {
    type Result = Rc<Record>;
    type Error = NativeError;
    fn field(&self, record: &Self::Result, field: &Field) -> Result<ScalarValue, NativeError> {
        self.events.borrow_mut().push(field.0);
        match field.0 {
            "target" => Ok(ScalarValue::String(record.target.clone())),
            "caption" => Ok(ScalarValue::String(record.caption.clone())),
            "panic" => panic!("native projection unwind"),
            _ => Err(NativeError("private native payload".into())),
        }
    }
    fn member(
        &self,
        _: &Self::Result,
        _: &str,
        _: &Operation,
    ) -> Result<Self::Result, NativeError> {
        Err(NativeError("not a group".into()))
    }
}

struct EditorTypes;
impl PureTypeEnvironment<Field, Operation> for EditorTypes {
    type Result = ();
    fn field_type(&self, _: &(), field: &Field) -> Option<ScalarType> {
        matches!(field.0, "target" | "caption").then_some(ScalarType::String)
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}

struct TextDomain {
    name: &'static str,
    events: Rc<RefCell<Vec<&'static str>>>,
}
impl ScalarArgumentDomain for TextDomain {
    fn scalar_types(&self) -> ScalarTypeSet {
        ScalarTypeSet::only(ScalarType::String)
    }
    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        self.events.borrow_mut().push(self.name);
        let ScalarValue::String(text) = value else {
            return false;
        };
        match self.name {
            "target-domain" => !text.is_empty() && !text.contains(' '),
            "panic-domain" => panic!("native domain unwind"),
            _ => text.len() <= 64,
        }
    }
}

struct CaptionDeclaration(Rc<()>);
impl HostResultDomain<Record> for CaptionDeclaration {
    type Error = ();
    fn matches_type(&self, _: &Record) -> bool {
        true
    }
    fn validate_value(&self, record: &Record) -> Result<(), ()> {
        (Rc::ptr_eq(&self.0, &record.owner) && record.caption.len() <= 64)
            .then_some(())
            .ok_or(())
    }
}

fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(name: &'static str) -> Ir {
    Ir::Field {
        value: Box::new(local("reply")),
        field: Field(name),
    }
}
fn literal(value: ScalarValue) -> Ir {
    Ir::Literal { value }
}
fn argument(name: &str, value: Ir) -> ComputedArgument<Ir> {
    ComputedArgument {
        name: name.into(),
        value,
    }
}
fn call(arguments: Vec<ComputedArgument<Ir>>) -> Ir {
    Ir::Call {
        operation: Operation("editor.caption"),
        arguments,
    }
}
fn reordered_call() -> Ir {
    call(vec![
        argument("caption", field("caption")),
        argument("target", field("target")),
    ])
}
fn record(owner: &Rc<()>, target: &str, caption: &str) -> Rc<Record> {
    Rc::new(Record {
        owner: owner.clone(),
        target: target.into(),
        caption: caption.into(),
    })
}
fn editor() -> Editor {
    Editor {
        events: Rc::new(RefCell::new(Vec::new())),
    }
}
fn parameters(editor: &Editor) -> [NamedParameter<&'static str, TextDomain>; 3] {
    [
        NamedParameter::required(
            "target",
            TextDomain {
                name: "target-domain",
                events: editor.events.clone(),
            },
        ),
        NamedParameter::optional(
            "note",
            TextDomain {
                name: "note-domain",
                events: editor.events.clone(),
            },
        ),
        NamedParameter::required(
            "caption",
            TextDomain {
                name: "caption-domain",
                events: editor.events.clone(),
            },
        ),
    ]
}

#[test]
fn original_schema_links_static_call_actual_arguments_received_reply_and_pure_tail() {
    let expression = reordered_call();
    let tail = Ir::Binary {
        operator: BinaryOperator::Concat,
        left: Box::new(field("caption")),
        right: Box::new(literal(ScalarValue::String("!".into()))),
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schemas = [OperationSchema {
        key: Operation("editor.caption"),
        parameters: &parameters,
        result: CaptionDeclaration(owner.clone()),
        required_capability: "editor.write",
    }];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap();
    let inferred = infer_call_type(
        &expression,
        &[("reply", PureType::Result(()))],
        &CallTypeHost {
            catalog: &catalog,
            version: 7,
            granted: &["editor.write"],
            environment: &EditorTypes,
        },
        CallTypeLimits {
            pure: TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 8,
            },
            max_arguments: 4,
        },
    )
    .unwrap();
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let prepared = prepare_call_in_scope(
        &expression,
        &mut scope,
        &CallEvaluationHost {
            catalog: &catalog,
            version: 7,
            granted: &["editor.write"],
            environment: &editor,
        },
        &mut fuel,
        LIMITS,
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert!(std::ptr::eq(&prepared.schema().result, inferred));
    assert_eq!(
        prepared
            .arguments()
            .iter()
            .map(|value| (value.parameter_index, value.argument_index, value.name))
            .collect::<Vec<_>>(),
        [(0, 1, "target"), (2, 0, "caption")]
    );
    assert!(std::ptr::eq(
        prepared.arguments()[0].name,
        parameters[0].name
    ));
    assert_eq!(
        *editor.events.borrow(),
        ["target", "caption", "target-domain", "caption-domain"]
    );
    assert_eq!(fuel.remaining(), 91);
    // Invocation is explicitly caller-owned, not performed by preparation.
    let (schema, arguments) = prepared.into_parts();
    let mut values = arguments.into_iter();
    let ScalarValue::String(target) = values.next().unwrap().value else {
        panic!()
    };
    let ScalarValue::String(caption) = values.next().unwrap().value else {
        panic!()
    };
    let reply = Rc::new(Record {
        owner: owner.clone(),
        target,
        caption,
    });
    schema.check_result(reply.as_ref()).unwrap();
    let mut bindings = vec![("reply", PureValue::Result(reply))];
    let mut scope = ScopeFrame::new(&mut bindings);
    assert!(
        matches!(evaluate_pure_in_scope(&tail, &mut scope, &editor, &mut fuel, LIMITS.pure).unwrap(), PureValue::Scalar(ScalarValue::String(text)) if text == "ready!")
    );
    assert_eq!(fuel.remaining(), 83);
}

struct Device;
impl PureEvaluationEnvironment<u8, u32> for Device {
    type Result = ();
    type Error = u8;
    fn field(&self, _: &(), _: &u8) -> Result<ScalarValue, u8> {
        Err(1)
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), u8> {
        Err(1)
    }
}
struct Position;
impl HostResultDomain<ScalarValue> for Position {
    type Error = ();
    fn matches_type(&self, reply: &ScalarValue) -> bool {
        reply.scalar_type() == ScalarType::Integer
    }
    fn validate_value(&self, reply: &ScalarValue) -> Result<(), ()> {
        matches!(reply, ScalarValue::Integer(position) if *position <= 100)
            .then_some(())
            .ok_or(())
    }
}

#[test]
fn unrelated_numeric_device_schema_prepares_and_validates_without_product_types() {
    let expression: Computation<u8, u32, (), ()> = Computation::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value: Computation::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(Computation::Literal {
                    value: ScalarValue::Integer(40),
                }),
                right: Box::new(Computation::Literal {
                    value: ScalarValue::Integer(2),
                }),
            },
        }],
    };
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: Position,
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(
        3,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let prepared = prepare_call_in_scope(
        &expression,
        &mut scope,
        &CallEvaluationHost {
            catalog: &catalog,
            version: 3,
            granted: &[31],
            environment: &Device,
        },
        &mut fuel,
        LIMITS,
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert_eq!(prepared.arguments()[0].value, ScalarValue::Integer(42));
    assert_eq!(fuel.remaining(), 96);
    prepared
        .schema()
        .check_result(&ScalarValue::Integer(42))
        .unwrap();
    assert_eq!(
        prepared
            .schema()
            .check_result(&ScalarValue::String("42".into())),
        Err(HostResultError::TypeMismatch)
    );
}

#[test]
fn catalog_version_capability_and_operation_rejections_precede_value_queries_and_fuel() {
    let expression = reordered_call();
    let unknown = Ir::Call {
        operation: Operation("unknown"),
        arguments: vec![
            argument("target", field("target")),
            argument("caption", field("caption")),
        ],
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schemas = [OperationSchema {
        key: Operation("editor.caption"),
        parameters: &parameters,
        result: (),
        required_capability: "editor.write",
    }];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap();
    for (expression, version, expected) in [
        (&expression, 8, OperationCatalogError::UnsupportedVersion),
        (&expression, 7, OperationCatalogError::CapabilityDenied),
        (&unknown, 7, OperationCatalogError::UnknownOperation),
    ] {
        let mut bindings = vec![(
            "reply",
            PureValue::Result(record(&owner, "window", "ready")),
        )];
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let failure = prepare_call_in_scope(
            expression,
            &mut scope,
            &CallEvaluationHost {
                catalog: &catalog,
                version,
                granted: &[],
                environment: &editor,
            },
            &mut fuel,
            LIMITS,
        )
        .unwrap_err();
        assert!(
            matches!(failure, CallEvaluationError::Preflight(CallTypeError::Catalog(error)) if error == expected)
        );
        assert_eq!(fuel.remaining(), 100);
        assert!(editor.events.borrow().is_empty());
    }
}

#[test]
fn whole_argument_forest_is_checked_before_native_catalog_comparison() {
    struct Key;
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            panic!("must not compare native key")
        }
    }
    struct Environment;
    impl PureEvaluationEnvironment<(), Key> for Environment {
        type Result = ();
        type Error = ();
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> {
            panic!()
        }
        fn member(&self, _: &(), _: &str, _: &Key) -> Result<(), ()> {
            panic!()
        }
    }
    let expression: Computation<(), Key, (), ()> = Computation::Call {
        operation: Key,
        arguments: vec![
            ComputedArgument {
                name: "first".into(),
                value: Computation::Literal {
                    value: ScalarValue::Integer(1),
                },
            },
            ComputedArgument {
                name: "last".into(),
                value: Computation::Choose {
                    when: Box::new(Computation::Literal {
                        value: ScalarValue::Boolean(true),
                    }),
                    then: Box::new(Computation::Literal {
                        value: ScalarValue::Integer(2),
                    }),
                    otherwise: Box::new(Computation::Host {
                        effect: Box::new(()),
                    }),
                },
            },
        ],
    };
    let parameters = [
        NamedParameter::required("first", ScalarTypeSet::only(ScalarType::Integer)),
        NamedParameter::required("last", ScalarTypeSet::only(ScalarType::Integer)),
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
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        prepare_call_in_scope(
            &expression,
            &mut scope,
            &CallEvaluationHost {
                catalog: &catalog,
                version: 1,
                granted: &[()],
                environment: &Environment
            },
            &mut fuel,
            LIMITS
        ),
        Err(CallEvaluationError::Preflight(CallTypeError::Inference {
            argument_index: 1,
            error: PureTypeError::Impure
        }))
    ));
    assert_eq!(fuel.remaining(), 100);
}

#[test]
fn named_shape_and_aggregate_limits_precede_evaluation() {
    let expressions = [
        call(vec![
            argument("target", field("target")),
            argument("target", field("caption")),
        ]),
        call(vec![
            argument("target", field("target")),
            argument("unknown", field("caption")),
        ]),
        call(vec![argument("target", field("target"))]),
        reordered_call(),
    ];
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    for (index, expression) in expressions.iter().enumerate() {
        let Ir::Call { arguments, .. } = expression else {
            panic!()
        };
        let mut bindings = vec![(
            "reply",
            PureValue::Result(record(&owner, "window", "ready")),
        )];
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let limits = if index == 3 {
            CallEvaluationLimits {
                pure: PureEvaluationLimits {
                    max_nodes: 4,
                    ..LIMITS.pure
                },
                ..LIMITS
            }
        } else {
            LIMITS
        };
        let failure = evaluate_call_arguments_in_scope(
            arguments, &schema, &mut scope, &editor, &mut fuel, limits,
        )
        .unwrap_err();
        match (index, failure) {
            (
                0,
                CallEvaluationError::Preflight(CallTypeError::Names(
                    NamedArgumentError::DuplicateArgument { index: 1 },
                )),
            )
            | (
                1,
                CallEvaluationError::Preflight(CallTypeError::Names(
                    NamedArgumentError::UnknownArgument { index: 1 },
                )),
            )
            | (
                2,
                CallEvaluationError::Preflight(CallTypeError::Names(
                    NamedArgumentError::MissingArgument { parameter_index: 2 },
                )),
            )
            | (3, CallEvaluationError::Preflight(CallTypeError::Inference { .. })) => {}
            _ => panic!("unexpected preflight failure"),
        }
        assert_eq!(fuel.remaining(), 100);
        assert!(editor.events.borrow().is_empty());
    }
}

#[test]
fn dynamic_domain_checks_wait_for_all_values_and_never_return_partial_output() {
    let expression = reordered_call();
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "bad target", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let failure = evaluate_call_arguments_in_scope(
        arguments, &schema, &mut scope, &editor, &mut fuel, LIMITS,
    )
    .unwrap_err();
    assert!(matches!(
        failure,
        CallEvaluationError::Argument {
            parameter_index: 0,
            argument_index: 1,
            error: ArgumentTypeError::InvalidLiteralDomain
        }
    ));
    assert_eq!(
        *editor.events.borrow(),
        ["target", "caption", "target-domain"]
    );
    assert_eq!(fuel.remaining(), 92);
    assert_eq!(scope.len(), 1);
    assert!(!format!("{failure:?}").contains("bad target"));
}

#[test]
fn scalar_calculation_failure_keeps_its_type_and_original_argument_position() {
    let expression = call(vec![
        argument("caption", field("caption")),
        argument(
            "target",
            Ir::Unary {
                operator: UnaryOperator::ToString,
                value: Box::new(Ir::Unary {
                    operator: UnaryOperator::ParseInteger,
                    value: Box::new(literal(ScalarValue::String("secret".into()))),
                }),
            },
        ),
    ]);
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let failure = evaluate_call_arguments_in_scope(
        arguments,
        &schema,
        &mut scope,
        &editor,
        &mut Fuel::new(100),
        LIMITS,
    )
    .unwrap_err();
    assert!(editor.events.borrow().is_empty());
    assert!(!format!("{failure:?}").contains("secret"));
    let CallEvaluationError::Evaluation {
        parameter_index: 0,
        argument_index: 1,
        failure,
    } = failure
    else {
        panic!()
    };
    assert!(matches!(
        failure,
        CalculationFailure::Scalar(ScalarError::InvalidIntegerText)
    ));
    assert!(failure.is_recoverable());
}

#[test]
fn native_error_moves_out_unchanged_and_does_not_recover_as_a_scalar_error() {
    let expression = call(vec![
        argument("target", field("private")),
        argument("caption", field("caption")),
    ]);
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let failure = evaluate_call_arguments_in_scope(
        arguments,
        &schema,
        &mut scope,
        &editor,
        &mut Fuel::new(100),
        LIMITS,
    )
    .unwrap_err();
    assert_eq!(*editor.events.borrow(), ["private"]);
    assert!(std::error::Error::source(&failure).is_none());
    assert!(!format!("{failure:?}").contains("private"));
    let CallEvaluationError::Evaluation { failure, .. } = failure else {
        panic!()
    };
    assert!(!failure.is_recoverable());
    let CalculationFailure::External(PureEvaluationFault::Native(NativeError(message))) = failure
    else {
        panic!()
    };
    assert_eq!(message, "private native payload");
}

#[test]
fn native_unwind_cleans_argument_locals_and_never_retries_projection() {
    let expression = call(vec![
        argument(
            "target",
            Ir::Bind {
                name: "temporary".into(),
                value: Box::new(literal(ScalarValue::Integer(1))),
                body: Box::new(field("panic")),
            },
        ),
        argument("caption", field("caption")),
    ]);
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = evaluate_call_arguments_in_scope(
                arguments, &schema, &mut scope, &editor, &mut fuel, LIMITS,
            );
        }))
        .is_err()
    );
    assert_eq!(*editor.events.borrow(), ["panic"]);
    assert_eq!(scope.len(), 1);
    assert!(scope.get("temporary").is_none());
    assert_eq!(fuel.remaining(), 96);
}

#[test]
fn fuel_exhaustion_stops_before_any_domain_callback_or_next_argument() {
    let expression = reordered_call();
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(4);
    let failure = evaluate_call_arguments_in_scope(
        arguments, &schema, &mut scope, &editor, &mut fuel, LIMITS,
    )
    .unwrap_err();
    assert!(matches!(
        failure,
        CallEvaluationError::Evaluation {
            parameter_index: 2,
            argument_index: 0,
            failure: CalculationFailure::External(PureEvaluationFault::FuelExhausted)
        }
    ));
    assert_eq!(fuel.remaining(), 0);
    assert_eq!(*editor.events.borrow(), ["target"]);
}

#[test]
fn optional_presence_keeps_empty_text_distinct_from_an_explicit_null() {
    for (note, accepted) in [
        (ScalarValue::String(String::new()), true),
        (
            ScalarValue::OptionalString(OptionalStringValue(None)),
            false,
        ),
    ] {
        let expression = call(vec![
            argument("caption", literal(ScalarValue::String("ready".into()))),
            argument("note", literal(note)),
            argument("target", literal(ScalarValue::String("window".into()))),
        ]);
        let Ir::Call { arguments, .. } = &expression else {
            panic!()
        };
        let editor = editor();
        let parameters = parameters(&editor);
        let schema = OperationSchema {
            key: (),
            parameters: &parameters,
            result: (),
            required_capability: (),
        };
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let result = evaluate_call_arguments_in_scope(
            arguments,
            &schema,
            &mut scope,
            &editor,
            &mut Fuel::new(100),
            LIMITS,
        );
        if accepted {
            let prepared = result.unwrap();
            assert_eq!(prepared.arguments().len(), 3);
            assert_eq!(prepared.arguments()[1].argument_index, 1);
            assert_eq!(prepared.arguments()[1].parameter_index, 1);
            assert_eq!(
                prepared.arguments()[1].value,
                ScalarValue::String(String::new())
            );
            assert_eq!(
                *editor.events.borrow(),
                ["target-domain", "note-domain", "caption-domain"]
            );
        } else {
            assert!(matches!(
                result,
                Err(CallEvaluationError::Argument {
                    parameter_index: 1,
                    argument_index: 1,
                    error: ArgumentTypeError::TypeMismatch,
                })
            ));
            assert_eq!(*editor.events.borrow(), ["target-domain"]);
        }
    }
}

#[test]
fn native_result_arguments_are_rejected_without_projecting_or_checking_domains() {
    let expression = call(vec![
        argument("caption", field("caption")),
        argument("target", local("reply")),
    ]);
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        evaluate_call_arguments_in_scope(
            arguments, &schema, &mut scope, &editor, &mut fuel, LIMITS,
        ),
        Err(CallEvaluationError::NonScalar {
            parameter_index: 0,
            argument_index: 1,
        })
    ));
    assert!(editor.events.borrow().is_empty());
    assert_eq!(scope.len(), 1);
    assert_eq!(fuel.remaining(), 99);
}

#[test]
fn domain_unwind_occurs_after_values_without_retry_or_lexical_leak() {
    let expression = call(vec![
        argument("caption", field("caption")),
        argument(
            "target",
            Ir::Bind {
                name: "temporary".into(),
                value: Box::new(literal(ScalarValue::Integer(1))),
                body: Box::new(field("target")),
            },
        ),
    ]);
    let Ir::Call { arguments, .. } = &expression else {
        panic!()
    };
    let owner = Rc::new(());
    let editor = editor();
    let mut parameters = parameters(&editor);
    parameters[0].domain.name = "panic-domain";
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "ready")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = evaluate_call_arguments_in_scope(
                arguments,
                &schema,
                &mut scope,
                &editor,
                &mut Fuel::new(100),
                LIMITS,
            );
        }))
        .is_err()
    );
    assert_eq!(
        *editor.events.borrow(),
        ["target", "caption", "panic-domain"]
    );
    assert_eq!(scope.len(), 1);
    assert!(scope.get("temporary").is_none());
}

#[test]
fn zero_argument_call_charges_only_its_root_and_selected_signature_charges_no_root() {
    let expression: Computation<u8, u32, (), ()> = Computation::Call {
        operation: 17,
        arguments: vec![],
    };
    let schemas: [OperationSchema<'_, u32, &str, ScalarTypeSet, (), u8>; 1] = [OperationSchema {
        key: 17,
        parameters: &[],
        result: (),
        required_capability: 31,
    }];
    let catalog = OperationCatalog::new(
        3,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let limits = CallEvaluationLimits {
        pure: PureEvaluationLimits {
            max_nodes: 1,
            max_depth: 0,
            max_bindings: 0,
        },
        max_arguments: 0,
    };
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(2);
    let prepared = prepare_call_in_scope(
        &expression,
        &mut scope,
        &CallEvaluationHost {
            catalog: &catalog,
            version: 3,
            granted: &[31],
            environment: &Device,
        },
        &mut fuel,
        limits,
    )
    .unwrap();
    assert!(prepared.arguments().is_empty());
    assert_eq!(fuel.remaining(), 1);
    let Computation::Call { arguments, .. } = &expression else {
        panic!()
    };
    assert!(
        evaluate_call_arguments_in_scope(
            arguments,
            &schemas[0],
            &mut scope,
            &Device,
            &mut fuel,
            limits
        )
        .unwrap()
        .arguments()
        .is_empty()
    );
    assert_eq!(fuel.remaining(), 1);
}

#[test]
fn prepared_output_and_policy_debug_are_metadata_only() {
    let expression = reordered_call();
    let owner = Rc::new(());
    let editor = editor();
    let parameters = parameters(&editor);
    let schemas = [OperationSchema {
        key: Operation("editor.caption"),
        parameters: &parameters,
        result: CaptionDeclaration(owner.clone()),
        required_capability: "private capability",
    }];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap();
    let host = CallEvaluationHost {
        catalog: &catalog,
        version: 7,
        granted: &["private capability"],
        environment: &editor,
    };
    let mut bindings = vec![(
        "reply",
        PureValue::Result(record(&owner, "window", "secret")),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let prepared =
        prepare_call_in_scope(&expression, &mut scope, &host, &mut Fuel::new(100), LIMITS).unwrap();
    for text in [
        format!("{host:?}"),
        format!("{prepared:?}"),
        format!("{:?}", prepared.arguments()),
    ] {
        for private in ["secret", "window", "private capability", "editor.caption"] {
            assert!(!text.contains(private));
        }
    }
}
