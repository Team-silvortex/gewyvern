use std::cell::{Cell, RefCell};
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_body::{HelperBodyError, HelperBodyLimits};
use leselang_hir::helper_body_source::*;
use leselang_hir::helper_instance::{HelperInstanceLimits, SelectedHelper};
use leselang_hir::helper_instance_finish::*;
use leselang_hir::helper_source::HelperParameterError;
use leselang_hir::helper_templates::HelperTemplateLimits;
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::PureTypeError;
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::{SourceCallLimits, SourceShapeError};
use leselang_hir::source_cost::{SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Function, NamedArgument, Span, parse};

type Data = Computation<(), (), (), ()>;
struct PrivateError(&'static str);
type Preparation =
    HelperBodySourceResult<Data, ScalarType, PrivateError, PrivateError, PrivateError>;
const LIMITS: HelperBodySourceLimits = HelperBodySourceLimits {
    max_source_nodes: 128,
    max_source_depth: 16,
    max_arguments: 64,
    body: HelperBodyLimits {
        template: HelperTemplateLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
            max_parameters: 8,
        },
        source_cost: SourceCostLimits {
            max_nodes: 128,
            max_depth: 16,
        },
    },
};
fn function(source: &str) -> Function {
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap()
}
fn integer(value: u64) -> Data {
    Data::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn observed(expression: Data) -> HelperBodyObservation<Data, ScalarType> {
    HelperBodyObservation {
        expression,
        result_type: ScalarType::Integer,
        scalar_result: Some(ScalarType::Integer),
    }
}
fn denied<T, L, V, C>(
    result: Result<T, HelperBodySourceError<L, V, C>>,
) -> HelperBodySourceError<L, V, C> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected helper source denial"),
    }
}
fn forbid(function: &Function, limits: HelperBodySourceLimits, used: &mut usize) -> Preparation {
    lower_helper_body(
        function,
        limits,
        used,
        |_, _, _| panic!("lowering must not run"),
        |_, _, _, _| panic!("validation must not run"),
        |_| panic!("cost must not run"),
    )
}
// Move-only native state and errors deliberately implement no Clone/Debug/serde/Send.
struct Native {
    bytes: Vec<u8>,
    drops: Rc<RefCell<Vec<&'static str>>>,
    id: &'static str,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
type NativeNode = Computation<Native, Native, Native, Native>;
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        bytes: vec![71; 31],
        drops: drops.clone(),
        id,
    }
}
fn native_observation(
    drops: &Rc<RefCell<Vec<&'static str>>>,
) -> HelperBodyObservation<NativeNode, Native> {
    HelperBodyObservation {
        expression: NativeNode::Host {
            effect: Box::new(native("effect", drops)),
        },
        result_type: native("result", drops),
        scalar_result: None,
    }
}

#[test]
fn six_closed_scalar_tokens_and_original_declaration_order_own_the_signature() {
    for (token, ty) in [
        ("integer", ScalarType::Integer),
        ("boolean", ScalarType::Boolean),
        ("string", ScalarType::String),
        ("none", ScalarType::None),
        ("optional_string", ScalarType::OptionalString),
        ("string_list", ScalarType::StringList),
    ] {
        let function = function(&format!("fn work(z: {token}, a: integer) = z"));
        let mut used = 7;
        let mut signature_buffer = std::ptr::null();
        let prepared = lower_helper_body(
            &function,
            LIMITS,
            &mut used,
            |original, parameters, counter| {
                assert!(std::ptr::eq(original, &function));
                assert_eq!(
                    parameters
                        .iter()
                        .map(|row| (row.name, row.domain))
                        .collect::<Vec<_>>(),
                    [("z", ty), ("a", ScalarType::Integer)]
                );
                assert_eq!(
                    parameters[0].name.as_ptr(),
                    function.parameters[0].name.as_ptr()
                );
                assert_eq!(*counter, 7);
                *counter += 1;
                Ok::<_, PrivateError>(HelperBodyObservation {
                    expression: Data::Local {
                        name: parameters[0].name.into(),
                    },
                    result_type: ty,
                    scalar_result: Some(ty),
                })
            },
            |_, parameters, result, scalar| {
                signature_buffer = parameters.as_ptr();
                assert_eq!(*result, ty);
                assert_eq!(scalar, Some(ty));
                Ok::<_, PrivateError>(())
            },
            |_| -> Result<SourceCostExtra, PrivateError> { panic!() },
        )
        .unwrap();
        assert_eq!(used, 8);
        assert_eq!(prepared.template().parameters().as_ptr(), signature_buffer);
        assert_eq!(
            prepared.template().parameters(),
            [("z".into(), ty), ("a".into(), ScalarType::Integer)]
        );
        assert_eq!(prepared.source_cost().nodes, 1);
    }
}

#[test]
fn every_source_output_prefix_and_cost_ceiling_precedes_native_lowering() {
    let function = function("fn work() = 0");
    let mut invalid = Vec::new();
    let mut row = LIMITS;
    row.max_source_nodes = 16_385;
    invalid.push(row);
    let mut row = LIMITS;
    row.max_source_depth = 65;
    invalid.push(row);
    let mut row = LIMITS;
    row.max_arguments = 65;
    invalid.push(row);
    let mut row = LIMITS;
    row.body.template.max_nodes = 16_385;
    invalid.push(row);
    let mut row = LIMITS;
    row.body.template.max_depth = 65;
    invalid.push(row);
    let mut row = LIMITS;
    row.body.template.max_bindings = 1_025;
    invalid.push(row);
    let mut row = LIMITS;
    row.body.template.max_parameters = 9;
    invalid.push(row);
    let mut row = LIMITS;
    row.body.source_cost.max_nodes = 16_385;
    invalid.push(row);
    let mut row = LIMITS;
    row.body.source_cost.max_depth = 65;
    invalid.push(row);
    for row in invalid {
        let mut used = 3;
        assert!(matches!(
            denied(forbid(&function, row, &mut used)),
            HelperBodySourceError::InvalidLimits
        ));
        assert_eq!(used, 3);
    }
}

#[test]
fn malformed_headers_and_exact_unused_prefix_capacity_precede_callbacks() {
    let mut bad_name = function("fn work() = 0");
    bad_name.name = "bad name".into();
    assert!(matches!(
        denied(forbid(&bad_name, LIMITS, &mut 0)),
        HelperBodySourceError::InvalidName
    ));
    for (token, expected) in [("signed", false), ("integer", true)] {
        let mut row = function(&format!("fn work(n: {token}) = 0"));
        if expected {
            row.parameters[0].name = "bad name".into();
        }
        assert!(matches!(
            denied(forbid(&row, LIMITS, &mut 0)),
            HelperBodySourceError::Parameters(
                HelperParameterError::UnknownType { index: 0 }
                    | HelperParameterError::InvalidName { index: 0 }
            )
        ));
    }
    let mut duplicate = function("fn work(n: integer, m: integer) = 0");
    duplicate.parameters[1].name = "n".into();
    assert!(matches!(
        denied(forbid(&duplicate, LIMITS, &mut 0)),
        HelperBodySourceError::Parameters(HelperParameterError::DuplicateName { index: 1 })
    ));
    let row = function("fn work(unused: string) = 0");
    let mut limits = LIMITS;
    limits.body.template.max_bindings = 0;
    assert!(matches!(
        denied(forbid(&row, limits, &mut 0)),
        HelperBodySourceError::ParameterScopeLimit
    ));
    limits = LIMITS;
    limits.body.template.max_parameters = 0;
    assert!(matches!(
        denied(forbid(&row, limits, &mut 0)),
        HelperBodySourceError::Parameters(HelperParameterError::ParameterLimit)
    ));
}

#[test]
fn complete_cold_source_names_text_arity_nodes_and_depth_precede_hooks() {
    let cold =
        function("fn work() = choose(when: true, then: 0, otherwise: native.work(value: 1))");
    let span = Span { start: 13, end: 19 };
    let cold_body = |value| Expression::Call {
        callee: "native.work".into(),
        arguments: vec![NamedArgument {
            name: "value".into(),
            value,
            span,
        }],
        span,
    };
    let mut bad_text = cold.clone();
    let Expression::Call { arguments, .. } = &mut bad_text.body else {
        panic!()
    };
    arguments[2].value = cold_body(Expression::String {
        value: "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        span,
    });
    let mut bad_name = cold.clone();
    let Expression::Call { arguments, .. } = &mut bad_name.body else {
        panic!()
    };
    arguments[2].value = cold_body(Expression::Reference {
        name: "bad name".into(),
        span,
    });
    for (row, expected) in [
        (bad_text, SourceShapeError::UnboundedText),
        (bad_name, SourceShapeError::InvalidName),
    ] {
        let mut used = 4;
        let error = denied(forbid(&row, LIMITS, &mut used));
        assert!(
            matches!(error, HelperBodySourceError::Source { span: actual, error } if actual == span && error == expected)
        );
        assert_eq!(used, 4);
    }
    let mut many = cold.clone();
    let Expression::Call { arguments, .. } = &mut many.body else {
        panic!()
    };
    arguments.extend(std::iter::repeat_n(arguments[0].clone(), 62));
    assert!(matches!(
        denied(forbid(&many, LIMITS, &mut 0)),
        HelperBodySourceError::Source {
            error: SourceShapeError::ArgumentLimit,
            ..
        }
    ));
    for (nodes, depth) in [(4, 16), (128, 1)] {
        let limits = HelperBodySourceLimits {
            max_source_nodes: nodes,
            max_source_depth: depth,
            ..LIMITS
        };
        assert!(matches!(
            denied(forbid(&cold, limits, &mut 0)),
            HelperBodySourceError::Source {
                error: SourceShapeError::Structure(_),
                ..
            }
        ));
    }
}

#[test]
fn zero_and_inclusive_limits_keep_source_and_lowered_policies_separate() {
    let function = function("fn work() = 7");
    let limits = HelperBodySourceLimits {
        max_source_nodes: 1,
        max_source_depth: 0,
        max_arguments: 0,
        body: HelperBodyLimits {
            template: HelperTemplateLimits {
                max_nodes: 1,
                max_depth: 0,
                max_bindings: 0,
                max_parameters: 0,
            },
            source_cost: SourceCostLimits {
                max_nodes: 1,
                max_depth: 0,
            },
        },
    };
    let mut used = 0;
    let prepared: Preparation = lower_helper_body(
        &function,
        limits,
        &mut used,
        |_, _, counter| {
            *counter += 1;
            Ok(observed(integer(7)))
        },
        |_, _, _, _| Ok(()),
        |_| panic!(),
    );
    assert_eq!(prepared.unwrap().source_cost().nodes, 1);
    assert_eq!(used, 1);
    assert!(matches!(
        denied(forbid(
            &function,
            HelperBodySourceLimits {
                max_source_nodes: 0,
                ..limits
            },
            &mut 0
        )),
        HelperBodySourceError::Source {
            error: SourceShapeError::Structure(StructureError::NodeLimit),
            ..
        }
    ));
    let mut limits = limits;
    limits.body.template.max_nodes = 0;
    let result: Preparation = lower_helper_body(
        &function,
        limits,
        &mut used,
        |_, _, _| Ok(observed(integer(7))),
        |_, _, _, _| panic!(),
        |_| panic!(),
    );
    assert!(matches!(
        denied(result),
        HelperBodySourceError::Admission(HelperBodyError::Template(_))
    ));
    assert_eq!(used, 1);
}

#[test]
fn lowerer_cannot_replace_signature_or_capture_undeclared_parameters() {
    let function = function("fn work(n: integer) = n");
    for value in [
        Data::Local {
            name: "invented".into(),
        },
        Data::Bind {
            name: "n".into(),
            value: Box::new(integer(1)),
            body: Box::new(integer(0)),
        },
    ] {
        let mut used = 5;
        let result: Preparation = lower_helper_body(
            &function,
            LIMITS,
            &mut used,
            |_, _, counter| {
                *counter += 1;
                Ok(observed(value))
            },
            |_, _, _, _| panic!(),
            |_| panic!(),
        );
        assert!(matches!(
            denied(result),
            HelperBodySourceError::Admission(HelperBodyError::Template(_))
        ));
        assert_eq!(used, 6);
    }
}

#[test]
fn forged_pure_return_and_zero_limit_cold_paths_fail_before_native_validation() {
    let function = function("fn work() = 0");
    for value in [
        Data::Choose {
            when: Box::new(Data::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(integer(0)),
            otherwise: Box::new(Data::Literal {
                value: ScalarValue::Boolean(false),
            }),
        },
        Data::Loop {
            name: "state".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Data::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(Data::Literal {
                value: ScalarValue::Boolean(true),
            }),
            limit: 0,
        },
    ] {
        let result: Preparation = lower_helper_body(
            &function,
            LIMITS,
            &mut 0,
            |_, _, counter| {
                *counter += 1;
                Ok(observed(value))
            },
            |_, _, _, _| panic!(),
            |_| panic!(),
        );
        assert!(matches!(
            denied(result),
            HelperBodySourceError::Admission(HelperBodyError::PureTyping(
                PureTypeError::BranchTypes | PureTypeError::LoopState
            ))
        ));
    }
    let mut false_type = observed(integer(7));
    false_type.scalar_result = Some(ScalarType::Boolean);
    let result: Preparation = lower_helper_body(
        &function,
        LIMITS,
        &mut 0,
        |_, _, _| Ok(false_type),
        |_, _, _, _| panic!(),
        |_| panic!(),
    );
    assert!(matches!(
        denied(result),
        HelperBodySourceError::Admission(HelperBodyError::PureResultMismatch { .. })
    ));
}

#[test]
fn original_counter_is_borrowed_once_and_successful_overcharge_drops_output_without_refund() {
    let function = function("fn work() = native.work()");
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut used = 7;
    let pointer = &mut used as *mut usize;
    let result = lower_helper_body(
        &function,
        LIMITS,
        &mut used,
        |_, _, counter| {
            assert_eq!(counter as *mut usize, pointer);
            *counter = usize::MAX;
            Ok::<_, PrivateError>(native_observation(&drops))
        },
        |_, _, _, _| -> Result<(), PrivateError> { panic!() },
        |_| -> Result<SourceCostExtra, PrivateError> { panic!() },
    );
    assert!(matches!(
        denied(result),
        HelperBodySourceError::SourceCounterLimit
    ));
    assert_eq!(used, usize::MAX);
    assert_eq!(*drops.borrow(), ["effect", "result"]);
    assert!(matches!(
        denied(forbid(&function, LIMITS, &mut used)),
        HelperBodySourceError::SourceCounterLimit
    ));
    assert_eq!(used, usize::MAX);
}

#[test]
fn once_lowering_validation_and_cost_keep_original_native_buffers_and_owned_policy() {
    let function = function("fn work(input: string) = native.work(value: input)");
    let drops = Rc::new(RefCell::new(Vec::new()));
    let output = native_observation(&drops);
    let NativeNode::Host { effect } = &output.expression else {
        panic!()
    };
    let original_box = &**effect as *const Native;
    let effect_buffer = effect.bytes.as_ptr();
    let result_buffer = output.result_type.bytes.as_ptr();
    let lowering_policy = native("lower-policy", &drops);
    let validating_policy = native("validate-policy", &drops);
    let calls = RefCell::new(Vec::new());
    let mut used = 3;
    let prepared = lower_helper_body(
        &function,
        LIMITS,
        &mut used,
        |original, parameters, counter| {
            calls.borrow_mut().push("lower");
            drop(lowering_policy);
            assert!(std::ptr::eq(original, &function));
            assert_eq!(parameters[0].name, "input");
            *counter += 2;
            Ok::<_, PrivateError>(output)
        },
        |body, parameters, result, scalar| {
            calls.borrow_mut().push("validate");
            drop(validating_policy);
            assert_eq!(parameters, [("input".into(), ScalarType::String)]);
            assert_eq!(result.bytes.as_ptr(), result_buffer);
            assert!(scalar.is_none());
            let NativeNode::Host { effect } = body else {
                panic!()
            };
            assert_eq!(&**effect as *const Native, original_box);
            Ok::<_, PrivateError>(())
        },
        |effect| {
            calls.borrow_mut().push("cost");
            assert_eq!(effect as *const Native, original_box);
            assert_eq!(effect.bytes.as_ptr(), effect_buffer);
            Ok::<_, PrivateError>(SourceCostExtra { nodes: 1, depth: 1 })
        },
    )
    .unwrap();
    assert_eq!(*calls.borrow(), ["lower", "validate", "cost"]);
    assert_eq!(used, 5);
    assert_eq!(prepared.source_cost().nodes, 2);
    assert_eq!(
        prepared.template().result_type().bytes.as_ptr(),
        result_buffer
    );
    assert_eq!(*drops.borrow(), ["lower-policy", "validate-policy"]);
    drop(prepared);
    assert_eq!(
        *drops.borrow(),
        ["lower-policy", "validate-policy", "effect", "result"]
    );
}

#[test]
fn validation_cost_errors_and_unwind_release_outputs_without_partial_body_or_counter_refund() {
    let function = function("fn work() = native.work()");
    for stage in 0..4 {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let validations = Cell::new(0);
        let costs = Cell::new(0);
        let mut used = 4;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            lower_helper_body(
                &function,
                LIMITS,
                &mut used,
                |_, _, counter| {
                    *counter += 1;
                    Ok::<_, PrivateError>(native_observation(&drops))
                },
                |_, _, _, _| {
                    validations.set(validations.get() + 1);
                    if stage == 0 {
                        return Err(PrivateError("private-validation"));
                    }
                    if stage == 2 {
                        panic!("native-validation-unwind");
                    }
                    Ok(())
                },
                |_| {
                    costs.set(costs.get() + 1);
                    if stage == 3 {
                        panic!("native-cost-unwind");
                    }
                    Err(PrivateError("private-cost"))
                },
            )
        }));
        if stage < 2 {
            let error = denied(outcome.unwrap());
            assert!(!format!("{error:?}: {error}").contains("private-"));
            assert!(error.source().is_none());
            assert!(matches!(
                error,
                HelperBodySourceError::Admission(
                    HelperBodyError::Validation(PrivateError("private-validation"))
                        | HelperBodyError::Cost(
                            leselang_hir::source_cost::SourceCostError::Observation {
                                host_index: 0,
                                error: PrivateError("private-cost")
                            }
                        )
                )
            ));
        } else {
            assert!(outcome.is_err());
        }
        assert_eq!(validations.get(), 1);
        assert_eq!(costs.get(), usize::from(stage == 1 || stage == 3));
        assert_eq!(used, 5);
        assert_eq!(*drops.borrow(), ["effect", "result"]);
    }
}

#[test]
fn lowering_error_and_unwind_preserve_counter_and_never_enter_admission() {
    let function = function("fn work() = 0");
    for unwind in [false, true] {
        let mut used = 4;
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Preparation {
            lower_helper_body(
                &function,
                LIMITS,
                &mut used,
                |_, _, counter| {
                    *counter += 1;
                    if unwind {
                        panic!("lowering-unwind");
                    }
                    Err(PrivateError("private-lowering"))
                },
                |_, _, _, _| panic!(),
                |_| panic!(),
            )
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let error = denied(outcome.unwrap());
            assert_eq!(
                format!("{error}"),
                "native helper body source lowering failed"
            );
            assert!(error.source().is_none());
            assert!(matches!(
                error,
                HelperBodySourceError::Lowering(PrivateError("private-lowering"))
            ));
        }
        assert_eq!(used, 5);
    }
}

struct PureHost;
impl PureEvaluationEnvironment<(), ()> for PureHost {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &()) -> Result<(), ()> {
        Err(())
    }
}
impl HelperInstanceFinisher<(), (), (), (), ScalarType> for PureHost {
    type Error = ();
    fn admit_argument(&mut self, value: &Data, expected: ScalarType, _: usize) -> Result<(), ()> {
        let Data::Literal { value } = value else {
            return Err(());
        };
        if value.scalar_type() == expected {
            Ok(())
        } else {
            Err(())
        }
    }
    fn argument_depth(&mut self, _: &Data) -> Result<usize, ()> {
        Ok(0)
    }
    fn materialize(&mut self, body: &Data) -> Result<Data, ()> {
        Ok(body.clone())
    }
    fn fresh_name(&mut self) -> Result<String, ()> {
        Ok("_arg".into())
    }
    fn admit(&mut self, value: &Data, result: &ScalarType) -> Result<(), ()> {
        let ty = leselang_hir::pure_typing::infer_pure_type(
            value,
            &[],
            &self::ScalarHost,
            leselang_hir::pure_typing::TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16,
            },
        )
        .map_err(|_| ())?;
        if ty == leselang_hir::pure_typing::PureType::Scalar(*result) {
            Ok(())
        } else {
            Err(())
        }
    }
}
struct ScalarHost;
impl leselang_hir::pure_typing::PureTypeEnvironment<(), ()> for ScalarHost {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &()) -> Option<()> {
        None
    }
}

#[test]
fn parsed_scalar_source_preparation_instance_and_value_keep_exact_counter_fuel_and_scope() {
    let function = function("fn work(n: integer) = add(left: n, right: 1)");
    let mut body_counter = 10;
    let prepared: Preparation = lower_helper_body(
        &function,
        LIMITS,
        &mut body_counter,
        |original, parameters, counter| {
            let prefix = parameters
                .iter()
                .map(|parameter| (parameter.name, parameter.domain))
                .collect::<Vec<_>>();
            let (expression, ty) = lower_scalar_source_with_scope(
                &original.body,
                ScalarSourceLimits {
                    source: SourceCallLimits {
                        max_source_nodes: 128,
                        max_source_depth: 16,
                        max_lowered_nodes: 128,
                        max_lowered_depth: 16,
                        max_arguments: 64,
                    },
                    max_bindings: 16,
                },
                &prefix,
                |_, _| -> Result<(Data, Option<ScalarType>), ()> { Err(()) },
            )
            .map_err(|_| PrivateError("lower"))?;
            *counter += 3;
            Ok(HelperBodyObservation {
                expression,
                result_type: ty,
                scalar_result: Some(ty),
            })
        },
        |_, _, result, scalar| {
            if scalar == Some(*result) {
                Ok(())
            } else {
                Err(PrivateError("return"))
            }
        },
        |_| panic!(),
    );
    let prepared = prepared.unwrap();
    assert_eq!(body_counter, 13);
    assert_eq!(prepared.source_cost().nodes, 3);
    // This is a distinct caller compilation, with its call and literal already charged.
    let mut caller_counter = 2;
    let (value, result) = finish_helper_instance(
        SelectedHelper {
            name: "work",
            body: &prepared,
            reserved_names: &[],
            caller_depth: 0,
        },
        vec![HelperInstanceArgument {
            value: integer(41),
            scalar_type: Some(ScalarType::Integer),
        }],
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_source_nodes: 128,
                max_source_depth: 16,
                max_lowered_nodes: 128,
                max_lowered_depth: 16,
                max_arguments: 64,
            },
            max_parameters: 8,
            max_bindings: 16,
            max_reserved_names: 128,
        },
        &mut caller_counter,
        &mut PureHost,
    )
    .unwrap();
    assert!(std::ptr::eq(result, prepared.template().result_type()));
    assert_eq!(caller_counter, 6);
    let mut fuel = Fuel::new(100);
    let mut bindings = vec![(
        "outside",
        leselang_hir::pure_evaluation::PureValue::Scalar(ScalarValue::Integer(9)),
    )];
    let result = evaluate_pure_in_scope(
        &value,
        &mut ScopeFrame::new(&mut bindings),
        &PureHost,
        &mut fuel,
        PureEvaluationLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
        },
    )
    .unwrap();
    assert!(matches!(
        result,
        leselang_hir::pure_evaluation::PureValue::Scalar(ScalarValue::Integer(42))
    ));
    assert_eq!(fuel.remaining(), 95);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "outside");
}

#[test]
fn reference_body_pipeline_keeps_wire_cold_authority_nested_helpers_and_error_priority() {
    let source = "fn outer(n: integer) = inner(n: inner(n: n))\nfn inner(n: integer) = add(left: n, right: 1)\nfn main() = ui.focus(node_id: to_string(value: outer(n: 40)))";
    let program = leselang_hir::lower(&parse(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let again = leselang_hir::lower(&parse(&canonical)).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&again).unwrap()
    );
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    for source in [
        "fn unused(n: integer) = choose(when: true, then: n, otherwise: false)\nfn main() = 0",
        "fn ready() = missing\nfn cycle() = cycle()\nfn main() = 0",
    ] {
        let tree = parse(source);
        let errors = leselang_hir::lower(&tree).unwrap_err();
        assert!(errors.iter().all(|row| row.code != "LSH1502"));
        assert!(
            errors
                .iter()
                .all(|row| row.span.is_some_and(|span| span.end <= source.len()))
        );
    }
    let source = "fn ready() = unknown.call(value: 1)\nfn main() = 0";
    let error = leselang_hir::lower(&parse(source)).unwrap_err();
    assert!(error.iter().any(|row| row.code == "LSH1003"));
}

#[test]
fn returned_scalar_category_still_requires_native_metadata_corroboration() {
    let function = function("fn work() = 7");
    let mut used = 0;
    let mut observation = observed(integer(7));
    observation.result_type = ScalarType::Boolean;
    let result: Preparation = lower_helper_body(
        &function,
        LIMITS,
        &mut used,
        |_, _, counter| {
            *counter += 1;
            Ok(observation)
        },
        |_, _, result, scalar| {
            if scalar == Some(*result) {
                Ok(())
            } else {
                Err(PrivateError("metadata-mismatch"))
            }
        },
        |_| panic!(),
    );
    assert!(matches!(
        denied(result),
        HelperBodySourceError::Admission(HelperBodyError::Validation(PrivateError(
            "metadata-mismatch"
        )))
    ));
    assert_eq!(used, 1);
}
