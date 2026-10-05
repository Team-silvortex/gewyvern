use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_source::*;
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::{SourceCallLimits, SourceShapeError};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Function, MAX_FUNCTION_PARAMETERS, NamedArgument, parse};

// All native IR slots are deliberately move-only and GUI-thread-local.
struct Native {
    id: &'static str,
    buffer: Vec<u8>,
    drops: Rc<RefCell<Vec<&'static str>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
struct PrivateError(&'static str);
type Node = Computation<Native, Native, Native, Native>;
const LIMITS: HelperSourceLimits = HelperSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    max_parameters: 8,
};
const TYPING: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
struct Host;
impl PureTypeEnvironment<Native, Native> for Host {
    type Result = ();
    fn field_type(&self, _: &(), field: &Native) -> Option<ScalarType> {
        (field.id == "field").then_some(ScalarType::Integer)
    }
    fn member_result(&self, _: &(), name: &str, operation: &Native) -> Option<()> {
        (name == "first" && operation.id == "operation").then_some(())
    }
}
impl PureEvaluationEnvironment<Native, Native> for Host {
    type Result = ();
    type Error = PrivateError;
    fn field(&self, _: &(), field: &Native) -> Result<ScalarValue, PrivateError> {
        if field.id == "field" {
            Ok(ScalarValue::Integer(9))
        } else {
            Err(PrivateError("private field"))
        }
    }
    fn member(&self, _: &(), name: &str, operation: &Native) -> Result<(), PrivateError> {
        if name == "first" && operation.id == "operation" {
            Ok(())
        } else {
            Err(PrivateError("private member"))
        }
    }
}
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        id,
        buffer: vec![3; 23],
        drops: drops.clone(),
    }
}
fn scalar(value: ScalarValue) -> (Node, Option<ScalarType>) {
    let ty = value.scalar_type();
    (Node::Literal { value }, Some(ty))
}
fn integer(value: u64) -> Node {
    scalar(ScalarValue::Integer(value)).0
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn expression(source: &str) -> Expression {
    let tree = parse(&format!("fn main() = {source}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn declaration(source: &str) -> Function {
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap()
}
fn arguments(expression: &Expression) -> &[NamedArgument] {
    let Expression::Call { arguments, .. } = expression else {
        panic!("expected call")
    };
    arguments
}
fn signature<'a>(parameters: &'a [HelperParameter<'a>]) -> HelperSignature<'a> {
    HelperSignature {
        name: "work",
        parameters,
    }
}
fn failed<T>(
    result: Result<T, HelperSourceError<PrivateError>>,
) -> HelperSourceError<PrivateError> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected helper rejection"),
    }
}
fn source_node(expression: &Expression, prefix: &[(&str, ScalarType)]) -> (Node, ScalarType) {
    lower_scalar_source_with_scope(
        expression,
        ScalarSourceLimits {
            source: LIMITS.source,
            max_bindings: 8,
        },
        prefix,
        |_, _| -> Result<_, PrivateError> { Err(PrivateError("unadapted source")) },
    )
    .unwrap()
}
fn evaluate(node: &Node, prefix: &[(&str, PureValue<()>)], fuel: &mut Fuel) -> ScalarValue {
    let mut bindings = prefix.to_vec();
    let mut scope = ScopeFrame::new(&mut bindings);
    let value = evaluate_pure_in_scope(node, &mut scope, &Host, fuel, PURE).unwrap();
    assert_eq!(scope.len(), prefix.len());
    for (name, original) in prefix {
        match (scope.get(name), original) {
            (Some(PureValue::Scalar(actual)), PureValue::Scalar(expected)) => {
                assert_eq!(actual, expected);
            }
            (Some(PureValue::Result(())), PureValue::Result(())) => {}
            _ => panic!("prefix changed"),
        }
    }
    match value {
        PureValue::Scalar(value) => value,
        _ => panic!("expected scalar"),
    }
}

#[test]
fn six_closed_signature_tokens_borrow_original_names_without_normalization() {
    let function = declaration(
        "fn work(i: integer, b: boolean, s: string, n: none, o: optional_string, l: string_list) = i",
    );
    let parameters = helper_parameters(&function, 6).unwrap();
    for ((parameter, source), ty) in parameters.iter().zip(&function.parameters).zip([
        ScalarType::Integer,
        ScalarType::Boolean,
        ScalarType::String,
        ScalarType::None,
        ScalarType::OptionalString,
        ScalarType::StringList,
    ]) {
        assert_eq!(parameter.domain, ty);
        assert!(parameter.required);
        assert_eq!(parameter.name.as_ptr(), source.name.as_ptr());
    }
    assert_eq!(
        helper_parameters(&declaration("fn work() = 0"), 0)
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn declaration_signature_errors_retain_indices_and_explicit_count_bounds() {
    let original = declaration("fn work(a: integer, b: boolean) = 0");
    for (name, type_name, expected) in [
        (
            "body",
            "integer",
            HelperParameterError::InvalidName { index: 1 },
        ),
        (
            "a",
            "boolean",
            HelperParameterError::DuplicateName { index: 1 },
        ),
        (
            "b",
            "Integer",
            HelperParameterError::UnknownType { index: 1 },
        ),
        (
            "b",
            " integer",
            HelperParameterError::UnknownType { index: 1 },
        ),
        (
            "b",
            "native_receipt",
            HelperParameterError::UnknownType { index: 1 },
        ),
    ] {
        let mut function = original.clone();
        function.parameters[1].name = name.into();
        function.parameters[1].type_name = type_name.into();
        assert_eq!(helper_parameters(&function, 8).unwrap_err(), expected);
    }
    assert_eq!(
        helper_parameters(&original, 1).unwrap_err(),
        HelperParameterError::ParameterLimit
    );
    assert_eq!(
        helper_parameters(&original, MAX_FUNCTION_PARAMETERS + 1).unwrap_err(),
        HelperParameterError::InvalidLimits
    );
    let mut mutated = original.clone();
    mutated.parameters = vec![mutated.parameters[0].clone(); 9];
    assert_eq!(
        helper_parameters(&mutated, 8).unwrap_err(),
        HelperParameterError::ParameterLimit
    );
}

#[test]
fn reordered_arguments_lower_once_in_declaration_order_with_original_borrows() {
    let parameters = [
        NamedParameter::required("left", ScalarType::Integer),
        NamedParameter::required("right", ScalarType::Integer),
    ];
    let source = expression("work(right: 4, left: 3)");
    let mut seen = Vec::new();
    let output = lower_helper_arguments(&source, signature(&parameters), LIMITS, |argument| {
        seen.push(argument.name.as_str());
        let Expression::Integer { value, .. } = argument.value else {
            panic!()
        };
        Ok::<_, PrivateError>(scalar(ScalarValue::Integer(value)))
    })
    .unwrap();
    assert_eq!(seen, ["left", "right"]);
    for (index, argument) in output.iter().enumerate() {
        assert_eq!(argument.parameter_index, index);
        assert_eq!(argument.argument_index, 1 - index);
        assert!(std::ptr::eq(argument.parameter, &parameters[index]));
        assert!(std::ptr::eq(
            argument.argument,
            &arguments(&source)[1 - index]
        ));
        assert_eq!(
            infer_pure_type(&argument.value, &[], &Host, TYPING),
            Ok(PureType::Scalar(ScalarType::Integer))
        );
    }
}

#[test]
fn parsed_arguments_compose_with_hygienic_body_without_leaking_parameter_scope_or_fuel() {
    let function = declaration("fn work(n: integer, m: integer) = add(left: n, right: m)");
    let parameters = helper_parameters(&function, 8).unwrap();
    let source = expression(
        "work(m: loop(acc: caller, while: lt(left: acc, right: 6), next: add(left: acc, right: 1), limit: 2), n: caller)",
    );
    let make_arguments = || {
        lower_helper_arguments(&source, signature(&parameters), LIMITS, |argument| {
            let (node, ty) = source_node(&argument.value, &[("caller", ScalarType::Integer)]);
            Ok::<_, PrivateError>((node, Some(ty)))
        })
        .unwrap()
    };
    let mut original = source_node(
        &function.body,
        &[("n", ScalarType::Integer), ("m", ScalarType::Integer)],
    )
    .0;
    for argument in make_arguments().into_iter().rev() {
        original = bind(argument.parameter.name, argument.value, original);
    }
    let mut renamed = hygienic_helper_body(
        source_node(
            &function.body,
            &[("n", ScalarType::Integer), ("m", ScalarType::Integer)],
        )
        .0,
        &[("n", "_n"), ("m", "_m")],
        &["caller"],
        HelperHygieneLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
            max_reserved_names: 8,
        },
        || -> Result<_, PrivateError> { panic!("body contains no local declarations") },
    )
    .unwrap();
    for (argument, alias) in make_arguments().into_iter().zip(["_n", "_m"]).rev() {
        renamed = bind(alias, argument.value, renamed);
    }
    let types = [("caller", PureType::Scalar(ScalarType::Integer))];
    assert_eq!(
        infer_pure_type(&renamed, &types, &Host, TYPING),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let values = [("caller", PureValue::Scalar(ScalarValue::Integer(5)))];
    let mut before = Fuel::new(1000);
    let mut after = Fuel::new(1000);
    assert_eq!(
        evaluate(&original, &values, &mut before),
        ScalarValue::Integer(11)
    );
    assert_eq!(
        evaluate(&renamed, &values, &mut after),
        ScalarValue::Integer(11)
    );
    assert_eq!(before.remaining(), after.remaining());
}

#[test]
fn all_six_argument_types_include_explicit_none_and_empty_optional_and_list() {
    let function = declaration(
        "fn work(i: integer, b: boolean, s: string, n: none, o: optional_string, l: string_list) = i",
    );
    let parameters = helper_parameters(&function, 8).unwrap();
    let source = expression(
        "work(l: strings(), o: optional_string(value: \"\"), n: none, s: \"\", b: false, i: 0)",
    );
    let output = lower_helper_arguments(&source, signature(&parameters), LIMITS, |argument| {
        let (node, ty) = source_node(&argument.value, &[]);
        Ok::<_, PrivateError>((node, Some(ty)))
    })
    .unwrap();
    for (argument, expected) in output.iter().zip([
        ScalarValue::Integer(0),
        ScalarValue::Boolean(false),
        ScalarValue::String("".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
        ScalarValue::StringList(StringListValue(vec![])),
    ]) {
        assert_eq!(
            infer_pure_type(&argument.value, &[], &Host, TYPING),
            Ok(PureType::Scalar(expected.scalar_type()))
        );
        assert_eq!(
            evaluate(&argument.value, &[], &mut Fuel::new(100)),
            expected
        );
    }
}

#[test]
fn complete_named_shape_and_required_signature_precede_all_callbacks() {
    let parameters = [
        NamedParameter::required("a", ScalarType::Integer),
        NamedParameter::required("b", ScalarType::Boolean),
    ];
    for call in [
        "work(a: 1)",
        "work(a: 1, c: false)",
        "work(a: 1, a: false)",
        "work(a: 1, b: false, extra: 1)",
    ] {
        let counter = Cell::new(0);
        let error = failed(lower_helper_arguments(
            &expression(call),
            signature(&parameters),
            LIMITS,
            |_| {
                counter.set(counter.get() + 1);
                Ok::<_, PrivateError>(scalar(ScalarValue::Integer(1)))
            },
        ));
        assert!(matches!(error, HelperSourceError::Names(_)));
        assert_eq!(counter.get(), 0);
    }
    for parameters in [
        vec![NamedParameter::optional("a", ScalarType::Integer)],
        vec![NamedParameter::required("body", ScalarType::Integer)],
        vec![
            NamedParameter::required("a", ScalarType::Integer),
            NamedParameter::required("a", ScalarType::Integer),
        ],
    ] {
        let error = failed(lower_helper_arguments(
            &expression("work(a: 1)"),
            signature(&parameters),
            LIMITS,
            |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!("invalid signature") },
        ));
        assert!(matches!(
            error,
            HelperSourceError::InvalidParameter { .. }
                | HelperSourceError::Names(NamedArgumentError::DuplicateParameter { .. })
        ));
    }
}

#[test]
fn callee_signature_and_explicit_limits_are_not_inferred_from_source() {
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    let error = failed(lower_helper_arguments(
        &expression("other(a: 1)"),
        signature(&parameters),
        LIMITS,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(error, HelperSourceError::WrongCallee));
    let error = failed(lower_helper_arguments(
        &expression("work(a: 1)"),
        HelperSignature {
            name: "body",
            parameters: &parameters,
        },
        LIMITS,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(error, HelperSourceError::InvalidSignature));
    let error = failed(lower_helper_arguments(
        &expression("1"),
        signature(&parameters),
        LIMITS,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(error, HelperSourceError::NotCall));
    let mut limits = LIMITS;
    limits.max_parameters = 9;
    let error = failed(lower_helper_arguments(
        &expression("work(a: 1)"),
        signature(&parameters),
        limits,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(error, HelperSourceError::InvalidLimits));
    limits.max_parameters = 0;
    let error = failed(lower_helper_arguments(
        &expression("work(a: 1)"),
        signature(&parameters),
        limits,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(error, HelperSourceError::ParameterLimit));
}

#[test]
fn physical_cold_source_bounds_reject_mutated_ast_before_native_lowering() {
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    let mut source = expression("work(a: choose(when: true, then: 1, otherwise: 2))");
    let mut limits = LIMITS;
    limits.source.max_source_nodes = 4;
    let error = failed(lower_helper_arguments(
        &source,
        signature(&parameters),
        limits,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(
        error,
        HelperSourceError::Shape {
            error: SourceShapeError::Structure(StructureError::NodeLimit),
            ..
        }
    ));
    let Expression::Call { arguments, .. } = &mut source else {
        panic!()
    };
    arguments[0].value = Expression::String {
        value: "x".repeat(4097),
        span: arguments[0].span,
    };
    let error = failed(lower_helper_arguments(
        &source,
        signature(&parameters),
        LIMITS,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(
        error,
        HelperSourceError::Shape {
            error: SourceShapeError::UnboundedText,
            ..
        }
    ));
    let mut limits = LIMITS;
    limits.source.max_source_depth = 0;
    let error = failed(lower_helper_arguments(
        &expression("work(a: 1)"),
        signature(&parameters),
        limits,
        |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() },
    ));
    assert!(matches!(
        error,
        HelperSourceError::Shape {
            error: SourceShapeError::Structure(StructureError::DepthLimit),
            ..
        }
    ));
}

#[test]
fn nested_native_source_arity_is_separate_from_helper_parameter_count() {
    let parameters = [NamedParameter::required("a", ScalarType::StringList)];
    let source = expression(
        "work(a: strings(a: \"1\", b: \"2\", c: \"3\", d: \"4\", e: \"5\", f: \"6\", g: \"7\", h: \"8\", i: \"9\"))",
    );
    let output = lower_helper_arguments(&source, signature(&parameters), LIMITS, |argument| {
        let (node, ty) = source_node(&argument.value, &[]);
        Ok::<_, PrivateError>((node, Some(ty)))
    })
    .unwrap();
    assert_eq!(output.len(), 1);
    assert!(
        matches!(evaluate(&output[0].value, &[], &mut Fuel::new(100)), ScalarValue::StringList(value) if value.0.len() == 9)
    );
}

#[test]
fn output_forest_counts_each_native_tree_once_without_phantom_call_or_wrapper_nodes() {
    let empty = expression("work()");
    let mut limits = LIMITS;
    limits.max_parameters = 0;
    limits.source.max_source_nodes = 1;
    limits.source.max_lowered_nodes = 0;
    limits.source.max_lowered_depth = 0;
    assert!(
        lower_helper_arguments(
            &empty,
            signature(&[]),
            limits,
            |_| -> Result<(Node, Option<ScalarType>), PrivateError> { panic!() }
        )
        .unwrap()
        .is_empty()
    );
    let parameters = [
        NamedParameter::required("a", ScalarType::Integer),
        NamedParameter::required("b", ScalarType::Integer),
    ];
    let source = expression("work(a: 1, b: 2)");
    let counter = Cell::new(0);
    limits = LIMITS;
    limits.source.max_lowered_nodes = 1;
    let error = failed(lower_helper_arguments(
        &source,
        signature(&parameters),
        limits,
        |_| {
            counter.set(counter.get() + 1);
            Ok::<_, PrivateError>(scalar(ScalarValue::Integer(1)))
        },
    ));
    assert!(matches!(
        error,
        HelperSourceError::Output {
            error: PureTypeError::Structure(StructureError::NodeLimit),
            ..
        }
    ));
    assert_eq!(counter.get(), 0);
    limits.source.max_lowered_nodes = 3;
    let error = failed(lower_helper_arguments(
        &source,
        signature(&parameters),
        limits,
        |_| {
            counter.set(counter.get() + 1);
            Ok::<_, PrivateError>((
                Node::Binary {
                    operator: BinaryOperator::Add,
                    left: Box::new(integer(1)),
                    right: Box::new(integer(2)),
                },
                Some(ScalarType::Integer),
            ))
        },
    ));
    assert!(matches!(
        error,
        HelperSourceError::Output {
            argument_index: 1,
            ..
        }
    ));
    assert_eq!(counter.get(), 1);
    limits.source.max_lowered_nodes = 2;
    limits.source.max_lowered_depth = 0;
    assert_eq!(
        lower_helper_arguments(&source, signature(&parameters), limits, |_| Ok::<
            _,
            PrivateError,
        >(
            scalar(ScalarValue::Integer(1))
        ))
        .unwrap()
        .len(),
        2
    );
}

#[test]
fn exact_scalar_facts_reject_non_scalar_mismatch_and_forged_literal_types() {
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    for (value, observation, expected) in [
        (ScalarValue::Integer(1), None, ArgumentTypeError::NonScalar),
        (
            ScalarValue::Boolean(true),
            Some(ScalarType::Boolean),
            ArgumentTypeError::TypeMismatch,
        ),
        (
            ScalarValue::Boolean(true),
            Some(ScalarType::Integer),
            ArgumentTypeError::InconsistentLiteralType,
        ),
    ] {
        let error = failed(lower_helper_arguments(
            &expression("work(a: 1)"),
            signature(&parameters),
            LIMITS,
            |_| {
                Ok::<_, PrivateError>((
                    Node::Literal {
                        value: value.clone(),
                    },
                    observation,
                ))
            },
        ));
        assert!(
            matches!(error, HelperSourceError::Argument { parameter_index: 0, argument_index: 0, error } if error == expected)
        );
    }
}

#[test]
fn native_projection_buffers_and_boxes_move_unchanged_through_cold_type_and_value_queries() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let field = native("field", &drops);
    let operation = native("operation", &drops);
    let field_pointer = field.buffer.as_ptr();
    let operation_pointer = operation.buffer.as_ptr();
    let input = Box::new(Node::Member {
        group: "group".into(),
        name: "first".into(),
        operation,
    });
    let input_pointer = &*input as *const Node;
    let mut node = Some(Node::Field {
        value: input,
        field,
    });
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    let source = expression("work(a: field(value: native_alias, name: \"total\"))");
    let output = lower_helper_arguments(&source, signature(&parameters), LIMITS, |_| {
        Ok::<_, PrivateError>((node.take().unwrap(), Some(ScalarType::Integer)))
    })
    .unwrap();
    let Node::Field { value, field } = &output[0].value else {
        panic!()
    };
    assert!(std::ptr::eq(&**value, input_pointer));
    assert_eq!(field.buffer.as_ptr(), field_pointer);
    let Node::Member { operation, .. } = &**value else {
        panic!()
    };
    assert_eq!(operation.buffer.as_ptr(), operation_pointer);
    assert_eq!(
        infer_pure_type(
            &output[0].value,
            &[("group", PureType::Result(()))],
            &Host,
            TYPING
        ),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(
        evaluate(
            &output[0].value,
            &[("group", PureValue::Result(()))],
            &mut Fuel::new(100)
        ),
        ScalarValue::Integer(9)
    );
    drop(output);
    let mut actual = drops.borrow().clone();
    actual.sort_unstable();
    assert_eq!(actual, ["field", "operation"]);
}

#[test]
fn cold_native_effect_slots_and_previously_lowered_operands_release_once_on_rejection() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let parameters = [
        NamedParameter::required("a", ScalarType::Integer),
        NamedParameter::required("b", ScalarType::Integer),
    ];
    let source = expression("work(b: 2, a: 1)");
    let counter = Cell::new(0);
    let error = failed(lower_helper_arguments(
        &source,
        signature(&parameters),
        LIMITS,
        |_| {
            let index = counter.get();
            counter.set(index + 1);
            let node = if index == 0 {
                Node::Field {
                    value: Box::new(local("result")),
                    field: native("field", &drops),
                }
            } else {
                Node::Choose {
                    when: Box::new(scalar(ScalarValue::Boolean(true)).0),
                    then: Box::new(integer(1)),
                    otherwise: Box::new(Node::Group {
                        group_kind: GroupKind::Sequence,
                        branches: vec![ComputedBranch {
                            name: "cold".into(),
                            result_type: native("result", &drops),
                            value: Node::Call {
                                operation: native("operation", &drops),
                                arguments: vec![ComputedArgument {
                                    name: "hidden".into(),
                                    value: Node::Host {
                                        effect: Box::new(native("effect", &drops)),
                                    },
                                }],
                            },
                        }],
                    }),
                }
            };
            Ok::<_, PrivateError>((node, Some(ScalarType::Integer)))
        },
    ));
    assert!(matches!(
        error,
        HelperSourceError::Argument {
            parameter_index: 1,
            argument_index: 0,
            error: ArgumentTypeError::Impure
        }
    ));
    assert_eq!(counter.get(), 2);
    let mut actual = drops.borrow().clone();
    actual.sort_unstable();
    assert_eq!(actual, ["effect", "field", "operation", "result"]);
}

#[test]
fn native_failure_and_unwind_keep_private_payloads_out_of_errors_and_do_not_retry() {
    let parameters = [
        NamedParameter::required("a", ScalarType::Integer),
        NamedParameter::required("b", ScalarType::Integer),
    ];
    let source = expression("work(a: 1, b: 2)");
    for unwind in [false, true] {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let count = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            lower_helper_arguments(&source, signature(&parameters), LIMITS, |_| {
                let index = count.get();
                count.set(index + 1);
                if index == 0 {
                    return Ok((
                        Node::Field {
                            value: Box::new(local("result")),
                            field: native("field", &drops),
                        },
                        Some(ScalarType::Integer),
                    ));
                }
                if unwind {
                    panic!("native allocator panic")
                }
                Err(PrivateError("secret callback payload"))
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            let error = failed(result.unwrap());
            assert!(!format!("{error:?} {error}").contains("secret"));
            assert!(std::error::Error::source(&error).is_none());
            assert!(matches!(
                error,
                HelperSourceError::Lowering {
                    argument_index: 1,
                    error: PrivateError("secret callback payload")
                }
            ));
        }
        assert_eq!(count.get(), 2);
        assert_eq!(*drops.borrow(), ["field"]);
        assert_eq!(
            lower_helper_arguments(&source, signature(&parameters), LIMITS, |_| Ok::<
                _,
                PrivateError,
            >(
                scalar(ScalarValue::Integer(1))
            ))
            .unwrap()
            .len(),
            2
        );
    }
}

#[test]
fn successful_dynamic_observations_do_not_replace_complete_cold_inference() {
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    let source = expression("work(a: 1)");
    for bad in [false, true] {
        let output = lower_helper_arguments(&source, signature(&parameters), LIMITS, |_| {
            let node = if bad {
                Node::Binary {
                    operator: BinaryOperator::Add,
                    left: Box::new(integer(1)),
                    right: Box::new(scalar(ScalarValue::Boolean(true)).0),
                }
            } else {
                local("unbound")
            };
            Ok::<_, PrivateError>((node, Some(ScalarType::Integer)))
        })
        .unwrap();
        assert_eq!(
            infer_pure_type(&output[0].value, &[], &Host, TYPING),
            Err(if bad {
                PureTypeError::BinaryOperands
            } else {
                PureTypeError::UnknownLocal
            })
        );
    }
}

#[test]
fn reference_signature_diagnostics_wire_names_and_authority_remain_compatible() {
    for (source, code, message, location) in [
        (
            "fn work(n: signed) = n\nfn main() = 0",
            "LSH1501",
            "expected integer, boolean, string, none, optional_string or string_list parameter type",
            0,
        ),
        (
            "fn work(n: integer) = n\nfn main() = work(other: 1)",
            "LSH1504",
            "helper arguments must match its named parameters exactly",
            1,
        ),
        (
            "fn work(n: integer) = n\nfn main() = work(n: false)",
            "LSH1504",
            "helper arguments must be pure scalars of the declared type",
            2,
        ),
    ] {
        let tree = parse(source);
        let declaration = |name| {
            tree.function
                .iter()
                .chain(&tree.helpers)
                .find(|function| function.name == name)
                .unwrap()
        };
        let expected_span = match location {
            0 => declaration("work").parameters[0].span,
            1 => {
                let Expression::Call { span, .. } = declaration("main").body else {
                    panic!()
                };
                span
            }
            _ => arguments(&declaration("main").body)[0].span,
        };
        let errors = leselang_hir::lower(&tree).unwrap_err();
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].message, message);
        assert_eq!(errors[0].span, Some(expected_span));
    }
    let source = "fn work(a: string, b: string) = ui.focus(node_id: concat(left: a, right: b))\nfn main() = work(b: \"b\", a: \"a\")";
    let program = leselang_hir::lower(&parse(source)).unwrap();
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        canonical,
        leselang_syntax::format(&parse("fn main() = bind(_lf0: \"a\", body: bind(_lf1: \"b\", body: ui.focus(node_id: concat(left: _lf0, right: _lf1))))")).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&leselang_hir::lower(&parse(&canonical)).unwrap()).unwrap()
    );
}

#[test]
fn cold_argument_labels_and_arity_are_checked_even_inside_native_owned_forms() {
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    for many in [false, true] {
        let mut source =
            expression("work(a: choose(when: true, then: 1, otherwise: native.form(key: 2)))");
        let Expression::Call { arguments, .. } = &mut source else {
            panic!()
        };
        let Expression::Call { arguments, .. } = &mut arguments[0].value else {
            panic!()
        };
        let Expression::Call { arguments, .. } = &mut arguments[2].value else {
            panic!()
        };
        if many {
            *arguments = vec![arguments[0].clone(); 65];
        } else {
            arguments[0].name = "bad-key".into();
        }
        let error = failed(lower_helper_arguments(
            &source,
            signature(&parameters),
            LIMITS,
            |_| -> Result<(Node, Option<ScalarType>), PrivateError> {
                panic!("cold source must fail first")
            },
        ));
        assert!(matches!(
            error,
            HelperSourceError::Shape {
                error: SourceShapeError::ArgumentLimit | SourceShapeError::InvalidName,
                ..
            }
        ));
    }
}

#[test]
fn native_output_bounds_and_language_names_are_not_certified_by_type_observations() {
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    let source = expression("work(a: 1)");
    for index in 0..4 {
        let mut limits = LIMITS;
        if index == 3 {
            limits.source.max_lowered_depth = 0;
        }
        let error = failed(lower_helper_arguments(
            &source,
            signature(&parameters),
            limits,
            |_| {
                let node = match index {
                    0 => scalar(ScalarValue::String("x".repeat(4097))).0,
                    1 => local("bad-key"),
                    2 => Node::Loop {
                        name: "acc".into(),
                        initial: Box::new(integer(0)),
                        condition: Box::new(scalar(ScalarValue::Boolean(false)).0),
                        next: Box::new(local("acc")),
                        limit: 1025,
                    },
                    _ => bind("a", integer(1), local("a")),
                };
                Ok::<_, PrivateError>((node, Some(ScalarType::Integer)))
            },
        ));
        let expected = match index {
            0 => PureTypeError::UnboundedLiteral,
            1 => PureTypeError::InvalidName,
            2 => PureTypeError::LoopLimit,
            _ => PureTypeError::Structure(StructureError::DepthLimit),
        };
        assert!(
            matches!(error, HelperSourceError::Output { argument_index: 0, error } if error == expected)
        );
    }
}
