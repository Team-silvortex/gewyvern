use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_bindings::{HelperBinding, HelperBindingLimits, bind_helper_arguments};
use leselang_hir::helper_body::*;
use leselang_hir::helper_expansion::HelperExpansionCost;
use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_returns::{HelperReturnLimits, HelperReturns};
use leselang_hir::helper_templates::{HelperTemplateError, HelperTemplateLimits};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::PureTypeError;
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::{SourceCostError, SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::*;

type Data = Computation<u8, u32, (), ()>;
const LIMITS: HelperBodyLimits = HelperBodyLimits {
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
};
fn literal(value: ScalarValue) -> Data {
    Data::Literal { value }
}
fn integer(value: u64) -> Data {
    literal(ScalarValue::Integer(value))
}
fn local(name: &str) -> Data {
    Data::Local { name: name.into() }
}
fn boolean(value: bool) -> Data {
    literal(ScalarValue::Boolean(value))
}
fn choose(cold: Data) -> Data {
    Data::Choose {
        when: Box::new(boolean(true)),
        then: Box::new(integer(1)),
        otherwise: Box::new(cold),
    }
}
fn bind(name: &str, value: Data, body: Data) -> Data {
    Data::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn lowered(expression: Data) -> LoweredHelperBody<Data, ScalarType> {
    LoweredHelperBody {
        parameters: vec![],
        expression,
        result_type: ScalarType::Integer,
        scalar_result: Some(ScalarType::Integer),
    }
}
fn denied<T, V, C>(result: Result<T, HelperBodyError<V, C>>) -> HelperBodyError<V, C> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected helper body denial"),
    }
}
fn accept(
    body: LoweredHelperBody<Data, ScalarType>,
) -> Result<HelperBody<Data, ScalarType>, HelperBodyError<(), ()>> {
    body.prepare(
        LIMITS,
        |_, _, result, scalar| {
            if scalar == Some(*result) {
                Ok(())
            } else {
                Err(())
            }
        },
        |_| Ok(SourceCostExtra { nodes: 0, depth: 0 }),
    )
}

// No native slot or result observation implements Clone, Debug, serde or Send.
struct Native {
    id: &'static str,
    bytes: Vec<u8>,
    drops: Rc<RefCell<Vec<&'static str>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
struct PrivateError(&'static str);
type NativeNode = Computation<Native, Native, Native, Native>;
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        id,
        bytes: vec![7; 31],
        drops: drops.clone(),
    }
}
fn native_body(drops: &Rc<RefCell<Vec<&'static str>>>) -> LoweredHelperBody<NativeNode, Native> {
    LoweredHelperBody {
        parameters: vec![("input".into(), ScalarType::String)],
        expression: NativeNode::Host {
            effect: Box::new(native("effect", drops)),
        },
        result_type: native("result", drops),
        scalar_result: None,
    }
}

#[test]
fn every_scalar_domain_is_independently_inferred_before_native_validation() {
    for value in [
        ScalarValue::Integer(42),
        ScalarValue::Boolean(true),
        ScalarValue::String("private".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
    ] {
        let ty = value.scalar_type();
        let body = LoweredHelperBody {
            parameters: vec![],
            expression: literal(value),
            result_type: ty,
            scalar_result: Some(ty),
        };
        let prepared = accept(body).unwrap();
        assert_eq!(*prepared.template().result_type(), ty);
        assert_eq!(prepared.template().shape().nodes, 1);
    }
}

#[test]
fn forged_pure_return_observation_is_denied_before_any_native_callback() {
    let mut body = lowered(integer(7));
    body.scalar_result = Some(ScalarType::Boolean);
    let error = denied(body.prepare(
        LIMITS,
        |_, _, _, _| -> Result<(), ()> { panic!("validation must not run") },
        |_| -> Result<SourceCostExtra, ()> { panic!("cost must not run") },
    ));
    assert!(matches!(
        error,
        HelperBodyError::PureResultMismatch {
            declared: ScalarType::Boolean,
            inferred: ScalarType::Integer,
        }
    ));
}

#[test]
fn pure_body_cannot_report_native_result_category() {
    let mut body = lowered(integer(7));
    body.scalar_result = None;
    assert!(matches!(
        denied(body.prepare(
            LIMITS,
            |_, _, _, _| -> Result<(), ()> { panic!() },
            |_| -> Result<SourceCostExtra, ()> { panic!() },
        )),
        HelperBodyError::PureResultRequired
    ));
}

#[test]
fn cold_choose_and_recovery_arms_require_complete_type_agreement() {
    for expression in [
        choose(boolean(false)),
        Data::Recover {
            value: Box::new(integer(0)),
            fallback: Box::new(boolean(false)),
        },
    ] {
        assert!(matches!(
            denied(accept(lowered(expression))),
            HelperBodyError::PureTyping(PureTypeError::BranchTypes | PureTypeError::RecoveryTypes)
        ));
    }
}

#[test]
fn zero_loop_and_fold_next_bodies_are_still_typed() {
    for expression in [
        Data::Loop {
            name: "state".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(boolean(false)),
            next: Box::new(boolean(true)),
            limit: 0,
        },
        Data::Fold {
            name: "state".into(),
            item: "item".into(),
            items: Box::new(Data::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(boolean(true)),
            limit: 0,
        },
    ] {
        assert!(matches!(
            denied(accept(lowered(expression))),
            HelperBodyError::PureTyping(PureTypeError::LoopState | PureTypeError::FoldState)
        ));
    }
}

#[test]
fn bounded_literals_and_loop_limits_fail_before_validation_even_when_cold() {
    let cold = [
        literal(ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1))),
        Data::Loop {
            name: "state".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(boolean(false)),
            next: Box::new(local("state")),
            limit: 1025,
        },
    ];
    for expression in cold {
        assert!(matches!(
            denied(lowered(choose(expression)).prepare(
                LIMITS,
                |_, _, _, _| -> Result<(), ()> { panic!() },
                |_| -> Result<SourceCostExtra, ()> { panic!() },
            )),
            HelperBodyError::PureTyping(PureTypeError::UnboundedLiteral | PureTypeError::LoopLimit)
        ));
    }
}

#[test]
fn cold_physical_and_lexical_preflight_precedes_purity_and_callbacks() {
    let mut oversized = integer(0);
    for _ in 0..17 {
        oversized = Data::Unary {
            operator: UnaryOperator::ToString,
            value: Box::new(oversized),
        };
    }
    for expression in [
        choose(local("missing")),
        choose(bind("bad name", integer(0), integer(0))),
        choose(oversized),
        Data::Group {
            group_kind: GroupKind::Parallel,
            branches: vec![ComputedBranch {
                name: "cold".into(),
                value: local("missing"),
                result_type: (),
            }],
        },
    ] {
        assert!(matches!(
            denied(lowered(expression).prepare(
                LIMITS,
                |_, _, _, _| -> Result<(), ()> { panic!() },
                |_| -> Result<SourceCostExtra, ()> { panic!() },
            )),
            HelperBodyError::Template(_)
        ));
    }
}

#[test]
fn exact_scalar_parameter_prefix_rejects_shadowing_and_unused_binding_overflow() {
    let mut body = lowered(bind("n", integer(0), local("n")));
    body.parameters = vec![("n".into(), ScalarType::Integer)];
    assert!(matches!(denied(accept(body)), HelperBodyError::Template(_)));
    let mut body = lowered(integer(0));
    body.parameters = vec![("unused".into(), ScalarType::Integer)];
    assert!(matches!(
        denied(body.prepare(
            HelperBodyLimits {
                template: HelperTemplateLimits {
                    max_bindings: 0,
                    ..LIMITS.template
                },
                ..LIMITS
            },
            |_, _, _, _| -> Result<(), ()> { panic!() },
            |_| -> Result<SourceCostExtra, ()> { panic!() },
        )),
        HelperBodyError::Template(_)
    ));
}

#[test]
fn parameter_types_are_used_instead_of_the_lowerer_return_guess() {
    let mut body = lowered(local("n"));
    body.parameters = vec![("n".into(), ScalarType::Boolean)];
    assert!(matches!(
        denied(accept(body)),
        HelperBodyError::PureResultMismatch {
            inferred: ScalarType::Boolean,
            ..
        }
    ));
}

#[test]
fn pure_native_projection_cannot_invent_a_result_from_scalar_parameters() {
    let mut body = lowered(Data::Field {
        value: Box::new(local("n")),
        field: 9,
    });
    body.parameters = vec![("n".into(), ScalarType::Integer)];
    assert!(matches!(
        denied(body.prepare(
            LIMITS,
            |_, _, _, _| -> Result<(), ()> { panic!() },
            |_| -> Result<SourceCostExtra, ()> { panic!() },
        )),
        HelperBodyError::PureTyping(_)
    ));
    let mut body = lowered(Data::Member {
        group: "n".into(),
        name: "part".into(),
        operation: 7,
    });
    body.parameters = vec![("n".into(), ScalarType::Integer)];
    assert!(matches!(
        denied(accept(body)),
        HelperBodyError::PureTyping(_)
    ));
}

#[test]
fn invalid_safety_ceilings_fail_before_native_validation_or_cost() {
    for limits in [
        HelperBodyLimits {
            source_cost: SourceCostLimits {
                max_nodes: 16_385,
                ..LIMITS.source_cost
            },
            ..LIMITS
        },
        HelperBodyLimits {
            source_cost: SourceCostLimits {
                max_depth: 65,
                ..LIMITS.source_cost
            },
            ..LIMITS
        },
        HelperBodyLimits {
            template: HelperTemplateLimits {
                max_parameters: 9,
                ..LIMITS.template
            },
            ..LIMITS
        },
    ] {
        assert!(matches!(
            denied(lowered(integer(0)).prepare(
                limits,
                |_, _, _, _| -> Result<(), ()> { panic!() },
                |_| -> Result<SourceCostExtra, ()> { panic!() },
            )),
            HelperBodyError::InvalidLimits
                | HelperBodyError::Template(HelperTemplateError::InvalidLimits)
        ));
    }
}

#[test]
fn zero_leaf_policies_and_folded_source_cost_are_independent_of_physical_shape() {
    let body = lowered(integer(0))
        .prepare(
            HelperBodyLimits {
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
            |_, _, _, _| Ok::<_, ()>(()),
            |_| Ok::<_, ()>(SourceCostExtra { nodes: 0, depth: 0 }),
        )
        .unwrap();
    assert_eq!(
        body.source_cost(),
        HelperExpansionCost { nodes: 1, depth: 0 }
    );
    let mut body = lowered(literal(ScalarValue::OptionalString(OptionalStringValue(
        None,
    ))));
    body.result_type = ScalarType::OptionalString;
    body.scalar_result = Some(ScalarType::OptionalString);
    let body = accept(body).unwrap();
    assert_eq!(body.template().shape().nodes, 1);
    assert_eq!(body.template().shape().depth, 0);
    assert_eq!(
        body.source_cost(),
        HelperExpansionCost { nodes: 2, depth: 1 }
    );
}

#[test]
fn native_validation_corroborates_result_metadata_after_shared_scalar_inference() {
    let mut body = lowered(integer(7));
    body.result_type = ScalarType::Boolean;
    let calls = Cell::new(0);
    assert!(matches!(
        denied(body.prepare(
            LIMITS,
            |_, _, result, observation| {
                calls.set(calls.get() + 1);
                assert_eq!(observation, Some(ScalarType::Integer));
                if observation == Some(*result) {
                    Ok(())
                } else {
                    Err(PrivateError("private-result"))
                }
            },
            |_| -> Result<SourceCostExtra, ()> { panic!() },
        )),
        HelperBodyError::Validation(PrivateError("private-result"))
    ));
    assert_eq!(calls.get(), 1);
}

#[test]
fn native_validator_can_consume_move_only_policy_as_fn_once_without_shared_state() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let policy = native("policy", &drops);
    let body = native_body(&drops)
        .prepare(
            LIMITS,
            move |_, _, _, _| {
                drop(policy);
                Ok::<_, PrivateError>(())
            },
            |_| Ok::<_, PrivateError>(SourceCostExtra { nodes: 0, depth: 0 }),
        )
        .unwrap();
    assert_eq!(*drops.borrow(), ["policy"]);
    drop(body);
    assert_eq!(*drops.borrow(), ["policy", "effect", "result"]);
}

#[test]
fn effectful_scalar_and_native_returns_remain_explicit_host_policy() {
    for observation in [None, Some(ScalarType::Integer)] {
        let body = LoweredHelperBody {
            parameters: vec![],
            expression: Data::Call {
                operation: 71,
                arguments: vec![],
            },
            result_type: 71u32,
            scalar_result: observation,
        };
        let admitted = body
            .prepare(
                LIMITS,
                |expression, parameters, result, scalar| {
                    assert!(matches!(expression, Data::Call { operation: 71, .. }));
                    assert!(parameters.is_empty());
                    assert_eq!(*result, 71);
                    assert_eq!(scalar, observation);
                    Ok::<_, ()>(())
                },
                |_| -> Result<SourceCostExtra, ()> { panic!() },
            )
            .unwrap();
        assert_eq!(
            admitted.source_cost(),
            HelperExpansionCost { nodes: 1, depth: 0 }
        );
    }
}

#[test]
fn all_move_only_native_slots_and_owned_buffers_are_handed_off_unchanged() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let parameters = vec![("input".to_owned(), ScalarType::String)];
    let signature_buffer = parameters.as_ptr();
    let name_buffer = parameters[0].0.as_ptr();
    let field = native("field", &drops);
    let field_buffer = field.bytes.as_ptr();
    let operation = native("operation", &drops);
    let operation_buffer = operation.bytes.as_ptr();
    let ir_result = native("ir-result", &drops);
    let result_buffer = ir_result.bytes.as_ptr();
    let arguments = vec![ComputedArgument {
        name: "payload".into(),
        value: NativeNode::Field {
            value: Box::new(NativeNode::Local {
                name: "input".into(),
            }),
            field,
        },
    }];
    let arguments_buffer = arguments.as_ptr();
    let branches = vec![
        ComputedBranch {
            name: "first".into(),
            value: NativeNode::Call {
                operation,
                arguments,
            },
            result_type: ir_result,
        },
        ComputedBranch {
            name: "second".into(),
            value: NativeNode::Host {
                effect: Box::new(native("effect", &drops)),
            },
            result_type: native("ir-second", &drops),
        },
    ];
    let branches_buffer = branches.as_ptr();
    let metadata = native("return", &drops);
    let metadata_buffer = metadata.bytes.as_ptr();
    let admitted = LoweredHelperBody {
        parameters,
        expression: NativeNode::Group {
            group_kind: GroupKind::Sequence,
            branches,
        },
        result_type: metadata,
        scalar_result: None,
    }
    .prepare(
        LIMITS,
        |body, signature, metadata, scalar| {
            assert_eq!(signature.as_ptr(), signature_buffer);
            assert_eq!(metadata.bytes.as_ptr(), metadata_buffer);
            assert_eq!(scalar, None);
            assert!(matches!(body, NativeNode::Group { .. }));
            Ok::<_, PrivateError>(())
        },
        |_| Ok::<_, PrivateError>(SourceCostExtra { nodes: 0, depth: 0 }),
    )
    .unwrap();
    assert_eq!(admitted.template().parameters()[0].0.as_ptr(), name_buffer);
    let (template, cost) = admitted.into_parts();
    assert_eq!(cost, HelperExpansionCost { nodes: 5, depth: 3 });
    let (parameters, body, result) = template.into_parts();
    let NativeNode::Group { branches, .. } = &body else {
        panic!()
    };
    assert_eq!(branches.as_ptr(), branches_buffer);
    assert_eq!(branches[0].result_type.bytes.as_ptr(), result_buffer);
    let NativeNode::Call {
        operation,
        arguments,
    } = &branches[0].value
    else {
        panic!()
    };
    assert_eq!(operation.bytes.as_ptr(), operation_buffer);
    assert_eq!(arguments.as_ptr(), arguments_buffer);
    let NativeNode::Field { field, .. } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), field_buffer);
    assert_eq!(result.bytes.as_ptr(), metadata_buffer);
    assert!(drops.borrow().is_empty());
    drop((parameters, body, result));
    assert_eq!(drops.borrow().len(), 6);
}

#[test]
fn original_host_box_is_borrowed_once_after_validation_and_not_dispatched() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let body = native_body(&drops);
    let NativeNode::Host { effect } = &body.expression else {
        panic!()
    };
    let pointer = effect.as_ref() as *const Native;
    let stage = Cell::new(0);
    let admitted = body
        .prepare(
            LIMITS,
            |body, _, _, _| {
                assert_eq!(stage.replace(1), 0);
                let NativeNode::Host { effect } = body else {
                    panic!()
                };
                assert!(std::ptr::eq(effect.as_ref(), pointer));
                Ok::<_, PrivateError>(())
            },
            |effect| {
                assert_eq!(stage.replace(2), 1);
                assert!(std::ptr::eq(effect, pointer));
                Ok::<_, PrivateError>(SourceCostExtra { nodes: 2, depth: 1 })
            },
        )
        .unwrap();
    assert_eq!(stage.get(), 2);
    assert_eq!(
        admitted.source_cost(),
        HelperExpansionCost { nodes: 3, depth: 1 }
    );
    assert!(drops.borrow().is_empty());
    drop(admitted);
    assert_eq!(drops.borrow().len(), 2);
}

#[test]
fn complete_cold_host_validation_precedes_rightmost_first_cost_observations() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let events = RefCell::new(Vec::new());
    let body = LoweredHelperBody {
        parameters: vec![],
        expression: NativeNode::Choose {
            when: Box::new(NativeNode::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(NativeNode::Host {
                effect: Box::new(native("selected", &drops)),
            }),
            otherwise: Box::new(NativeNode::Host {
                effect: Box::new(native("cold", &drops)),
            }),
        },
        result_type: (),
        scalar_result: None,
    };
    let admitted = body
        .prepare(
            LIMITS,
            |body, _, _, _| {
                assert_eq!(body.children().count(), 3);
                events.borrow_mut().push("validate-all");
                Ok::<_, ()>(())
            },
            |effect| {
                events.borrow_mut().push(effect.id);
                Ok::<_, ()>(SourceCostExtra { nodes: 0, depth: 0 })
            },
        )
        .unwrap();
    assert_eq!(*events.borrow(), ["validate-all", "cold", "selected"]);
    assert_eq!(admitted.source_cost().nodes, 4);
}

#[test]
fn validation_error_prevents_cost_and_partial_registry_entry_and_drops_inputs_once() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut registry = Vec::new();
    let outcome = native_body(&drops).prepare(
        LIMITS,
        |_, _, _, _| Err(PrivateError("secret-validation")),
        |_| -> Result<SourceCostExtra, PrivateError> { panic!() },
    );
    let error = match outcome {
        Ok(body) => {
            registry.push(body);
            panic!()
        }
        Err(error) => error,
    };
    assert!(registry.is_empty());
    assert!(matches!(
        &error,
        HelperBodyError::Validation(PrivateError("secret-validation"))
    ));
    assert!(!format!("{error:?} {error}").contains("secret-validation"));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(*drops.borrow(), ["effect", "result"]);
}

#[test]
fn cost_error_retains_private_error_without_retry_refund_or_partial_output() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let validation = Cell::new(0);
    let costs = Cell::new(0);
    let error = denied(native_body(&drops).prepare(
        LIMITS,
        |_, _, _, _| {
            validation.set(validation.get() + 1);
            Ok::<_, PrivateError>(())
        },
        |_| {
            costs.set(costs.get() + 1);
            Err(PrivateError("secret-cost"))
        },
    ));
    assert!(matches!(
        &error,
        HelperBodyError::Cost(SourceCostError::Observation {
            host_index: 0,
            error: PrivateError("secret-cost")
        })
    ));
    assert_eq!((validation.get(), costs.get()), (1, 1));
    assert!(!format!("{error:?} {error}").contains("secret-cost"));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(drops.borrow().len(), 2);
}

#[test]
fn validation_and_cost_unwind_drop_owned_inputs_once_without_catching_or_retrying() {
    for cost_panics in [false, true] {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let validations = Cell::new(0);
        let costs = Cell::new(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            native_body(&drops).prepare(
                LIMITS,
                |_, _, _, _| {
                    validations.set(validations.get() + 1);
                    assert!(cost_panics, "explicit native validation unwind");
                    Ok::<_, PrivateError>(())
                },
                |_| -> Result<SourceCostExtra, PrivateError> {
                    costs.set(costs.get() + 1);
                    panic!("explicit native cost unwind")
                },
            )
        }));
        assert!(outcome.is_err());
        assert_eq!(validations.get(), 1);
        assert_eq!(costs.get(), usize::from(cost_panics));
        assert_eq!(drops.borrow().len(), 2);
    }
}

#[test]
fn source_limits_deny_after_validation_without_undoing_native_validation_work() {
    let validations = Cell::new(0);
    for cost in [
        SourceCostLimits {
            max_nodes: 0,
            max_depth: 0,
        },
        SourceCostLimits {
            max_nodes: 1,
            max_depth: 0,
        },
    ] {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let calls = Cell::new(0);
        let error = denied(native_body(&drops).prepare(
            HelperBodyLimits {
                source_cost: cost,
                ..LIMITS
            },
            |_, _, _, _| {
                validations.set(validations.get() + 1);
                Ok::<_, ()>(())
            },
            |_| {
                calls.set(calls.get() + 1);
                Ok::<_, ()>(SourceCostExtra {
                    nodes: usize::MAX,
                    depth: usize::MAX,
                })
            },
        ));
        assert!(matches!(
            error,
            HelperBodyError::Cost(SourceCostError::Structure(_))
        ));
        assert_eq!(calls.get(), usize::from(cost.max_nodes != 0));
        assert_eq!(drops.borrow().len(), 2);
    }
    assert_eq!(validations.get(), 2);
}

#[test]
fn debug_only_exposes_counts_and_shape_not_private_data_or_native_observations() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut body = native_body(&drops);
    body.parameters[0].0 = "secret_parameter".into();
    let before = format!("{body:?}");
    let admitted = body
        .prepare(
            LIMITS,
            |_, _, _, _| Ok::<_, PrivateError>(()),
            |_| Ok::<_, PrivateError>(SourceCostExtra { nodes: 0, depth: 0 }),
        )
        .unwrap();
    let after = format!("{admitted:?}");
    for text in [before, after] {
        assert!(!text.contains("secret_parameter"));
        assert!(!text.contains("input"));
        assert!(!text.contains("effect"));
        assert!(!text.contains("result"));
    }
}

struct Numbers;
impl PureEvaluationEnvironment<u8, u32> for Numbers {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &u8) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), ()> {
        Err(())
    }
}
fn evaluate(body: &Data, fuel: &mut Fuel) -> ScalarValue {
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let result = evaluate_pure_in_scope(
        body,
        &mut scope,
        &Numbers,
        fuel,
        PureEvaluationLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
        },
    )
    .unwrap();
    assert!(scope.is_empty());
    let PureValue::Scalar(value) = result else {
        panic!()
    };
    value
}

#[test]
fn parsed_scalar_helper_admission_materialization_binding_and_returns_keep_value_and_exact_fuel() {
    let tree = leselang_syntax::parse("fn work(n: integer) = add(left: n, right: 1)");
    assert!(tree.diagnostics.is_empty());
    let helper = tree.function.unwrap();
    let (body, ty) = lower_scalar_source_with_scope(
        &helper.body,
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
        &[("n", ScalarType::Integer)],
        |_, _| -> Result<(Data, Option<ScalarType>), ()> { Err(()) },
    )
    .unwrap();
    let admitted = accept(LoweredHelperBody {
        parameters: vec![("n".into(), ScalarType::Integer)],
        expression: body,
        result_type: ty,
        scalar_result: Some(ty),
    })
    .unwrap();
    assert_eq!(
        admitted.source_cost(),
        HelperExpansionCost { nodes: 3, depth: 1 }
    );
    let original = bind("n", integer(41), admitted.template().body().clone());
    let original = bind("answer", original, local("answer"));
    let copied = admitted
        .template()
        .materialize(LIMITS.template, |body| Ok::<_, Infallible>(body.clone()))
        .unwrap();
    let copied = hygienic_helper_body(
        copied,
        &[("n", "_p")],
        &[],
        HelperHygieneLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
            max_reserved_names: 0,
        },
        || Err::<String, ()>(()),
    )
    .unwrap();
    let copied = bind_helper_arguments(
        copied,
        vec![HelperBinding {
            name: "_p".into(),
            value: integer(41),
        }],
        HelperBindingLimits {
            max_nodes: 128,
            max_depth: 16,
            max_parameters: 8,
        },
    )
    .unwrap();
    let composed = HelperReturns::new(
        "answer".into(),
        copied,
        local("answer"),
        HelperReturnLimits {
            max_nodes: 128,
            max_depth: 16,
        },
    )
    .unwrap()
    .connect(|body, _| Ok::<_, Infallible>(body.clone()))
    .unwrap();
    let mut before = Fuel::new(100);
    let mut after = Fuel::new(100);
    assert_eq!(evaluate(&original, &mut before), ScalarValue::Integer(42));
    assert_eq!(evaluate(&composed, &mut after), ScalarValue::Integer(42));
    assert_eq!(before.remaining(), after.remaining());
}

#[test]
fn reference_helpers_preserve_canonical_wire_authority_and_unused_cold_rejection() {
    let source = "fn work(n: integer) = add(left: n, right: 1)\nfn main() = ui.focus(node_id: to_string(value: work(n: 41)))";
    let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let again = leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&again).unwrap()
    );
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let invalid =
        "fn unused(n: integer) = choose(when: true, then: n, otherwise: false)\nfn main() = 0";
    assert!(leselang_hir::lower(&leselang_syntax::parse(invalid)).is_err());
}
