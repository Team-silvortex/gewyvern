use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{
    CallEvaluationHost, CallEvaluationLimits, prepare_call_in_scope,
};
use leselang_hir::helper_bindings::{HelperBinding, HelperBindingLimits, bind_helper_arguments};
use leselang_hir::helper_dependencies::{HelperDependencyLimits, helper_dependency_order};
use leselang_hir::helper_hygiene::{HelperHygieneError, HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_source::{
    HelperSignature, HelperSourceLimits, helper_parameters, lower_helper_arguments,
};
use leselang_hir::helper_templates::*;
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::*;

// Native fields, operations, effects, IR results and return metadata are move-only.
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
type Node = Computation<Native, Native, Native, Native>;
type Template = HelperTemplate<Node, Native>;
const LIMITS: HelperTemplateLimits = HelperTemplateLimits {
    max_nodes: 128,
    max_depth: 16,
    max_bindings: 16,
    max_parameters: 8,
};
const TYPING: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 128,
    max_depth: 16,
    max_bindings: 16,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 128,
    max_depth: 16,
    max_bindings: 16,
};
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        id,
        bytes: vec![7; 31],
        drops: drops.clone(),
    }
}
fn integer(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn parameters() -> Vec<(String, ScalarType)> {
    vec![("n".into(), ScalarType::Integer)]
}
fn choose(cold: Node) -> Node {
    Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(integer(0)),
        otherwise: Box::new(cold),
    }
}
fn bind(name: &str, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(integer(0)),
        body: Box::new(body),
    }
}
fn template(body: Node, drops: &Rc<RefCell<Vec<&'static str>>>) -> Template {
    Template::new(parameters(), body, native("return", drops), LIMITS).unwrap()
}
fn rejected<Error>(result: Result<Node, HelperTemplateError<Error>>) -> HelperTemplateError<Error> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected materialization rejection"),
    }
}
fn materialize(
    template: &Template,
    output: Node,
) -> Result<Node, HelperTemplateError<PrivateError>> {
    template.materialize(LIMITS, |_| Ok(output))
}

#[test]
fn owned_signature_body_and_return_metadata_move_unchanged_and_can_be_consumed() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let signature = parameters();
    let signature_buffer = signature.as_ptr();
    let name_buffer = signature[0].0.as_ptr();
    let value = Box::new(local("n"));
    let value_box = value.as_ref() as *const Node;
    let field = native("field", &drops);
    let field_buffer = field.bytes.as_ptr();
    let result = native("return", &drops);
    let result_buffer = result.bytes.as_ptr();
    let template = Template::new(signature, Node::Field { value, field }, result, LIMITS).unwrap();
    assert_eq!(template.parameters().as_ptr(), signature_buffer);
    assert_eq!(template.parameters()[0].0.as_ptr(), name_buffer);
    assert_eq!(template.result_type().bytes.as_ptr(), result_buffer);
    assert_eq!(template.shape(), HelperTemplateShape { nodes: 2, depth: 1 });
    let (signature, body, result) = template.into_parts();
    assert_eq!(signature.as_ptr(), signature_buffer);
    let Node::Field { value, field } = &body else {
        panic!()
    };
    assert_eq!(value.as_ref() as *const Node, value_box);
    assert_eq!(field.bytes.as_ptr(), field_buffer);
    assert_eq!(result.bytes.as_ptr(), result_buffer);
    assert!(drops.borrow().is_empty());
    drop((signature, body, result));
    assert_eq!(drops.borrow().len(), 2);
}

#[test]
fn zero_parameters_bindings_and_leaf_depth_are_explicit() {
    let limits = HelperTemplateLimits {
        max_nodes: 1,
        max_depth: 0,
        max_bindings: 0,
        max_parameters: 0,
    };
    let template: HelperTemplate<Node, ()> =
        HelperTemplate::new(vec![], integer(4), (), limits).unwrap();
    assert_eq!(template.shape(), HelperTemplateShape { nodes: 1, depth: 0 });
    assert!(
        template
            .materialize(limits, |_| Ok::<_, PrivateError>(integer(4)))
            .is_ok()
    );
    assert!(matches!(
        HelperTemplate::new(
            vec![],
            integer(0),
            (),
            HelperTemplateLimits {
                max_nodes: 0,
                ..limits
            }
        )
        .unwrap_err(),
        HelperTemplateError::Body(HelperHygieneError::Structure(StructureError::NodeLimit))
    ));
}

#[test]
fn invalid_limits_and_signature_indices_are_checked_before_body_inspection() {
    for limits in [
        HelperTemplateLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        HelperTemplateLimits {
            max_depth: 65,
            ..LIMITS
        },
        HelperTemplateLimits {
            max_bindings: 1025,
            ..LIMITS
        },
        HelperTemplateLimits {
            max_parameters: 9,
            ..LIMITS
        },
    ] {
        assert!(matches!(
            HelperTemplate::new(parameters(), local("missing"), (), limits).unwrap_err(),
            HelperTemplateError::InvalidLimits
        ));
    }
    for name in ["n", "bad name", "body"] {
        let error = HelperTemplate::new(
            vec![
                ("n".into(), ScalarType::Integer),
                (name.into(), ScalarType::Boolean),
            ],
            local("missing"),
            (),
            LIMITS,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            HelperTemplateError::InvalidParameter { index: 1 }
        ));
    }
}

#[test]
fn parameter_and_active_binding_quotas_include_unused_signature_entries() {
    let signature = || {
        (0..8)
            .map(|index| (format!("p{index}"), ScalarType::Integer))
            .collect()
    };
    assert!(
        HelperTemplate::new(
            signature(),
            local("p7"),
            (),
            HelperTemplateLimits {
                max_bindings: 8,
                ..LIMITS
            }
        )
        .is_ok()
    );
    assert!(matches!(
        HelperTemplate::new(
            signature(),
            integer(0),
            (),
            HelperTemplateLimits {
                max_bindings: 7,
                ..LIMITS
            }
        )
        .unwrap_err(),
        HelperTemplateError::Body(HelperHygieneError::BindingLimit)
    ));
    assert!(matches!(
        HelperTemplate::new(
            signature(),
            integer(0),
            (),
            HelperTemplateLimits {
                max_parameters: 7,
                ..LIMITS
            }
        )
        .unwrap_err(),
        HelperTemplateError::ParameterLimit
    ));
    assert!(matches!(
        HelperTemplate::new(
            parameters(),
            bind("x", local("x")),
            (),
            HelperTemplateLimits {
                max_bindings: 1,
                ..LIMITS
            }
        )
        .unwrap_err(),
        HelperTemplateError::Body(HelperHygieneError::BindingLimit)
    ));
}

#[test]
fn every_cold_free_local_group_and_active_shadow_is_rejected_at_storage() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    for body in [choose(local("missing")), choose(bind("n", local("n")))] {
        assert!(matches!(
            Template::new(parameters(), body, native("return", &drops), LIMITS).unwrap_err(),
            HelperTemplateError::Body(
                HelperHygieneError::Capture { .. } | HelperHygieneError::ShadowedBinding
            )
        ));
    }
    let body = choose(Node::Member {
        group: "missing".into(),
        name: "one".into(),
        operation: native("operation", &drops),
    });
    assert!(matches!(
        Template::new(parameters(), body, native("return", &drops), LIMITS).unwrap_err(),
        HelperTemplateError::Body(HelperHygieneError::Capture { group: true })
    ));
    assert_eq!(drops.borrow().len(), 4);
}

#[test]
fn zero_loop_and_fold_paths_still_have_complete_lexical_preflight() {
    for body in [
        Node::Loop {
            name: "state".into(),
            initial: Box::new(local("n")),
            condition: Box::new(Node::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(local("missing")),
            limit: 0,
        },
        Node::Fold {
            name: "state".into(),
            item: "part".into(),
            items: Box::new(Node::Strings { items: vec![] }),
            initial: Box::new(local("n")),
            next: Box::new(local("missing")),
            limit: 0,
        },
    ] {
        assert!(matches!(
            HelperTemplate::new(parameters(), body, (), LIMITS).unwrap_err(),
            HelperTemplateError::Body(HelperHygieneError::Capture { group: false })
        ));
    }
}

#[test]
fn physical_frontier_and_depth_bounds_precede_lexical_scope_walks() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let body = Node::Call {
        operation: native("operation", &drops),
        arguments: (0..64)
            .map(|i| ComputedArgument {
                name: format!("a{i}"),
                value: local("missing"),
            })
            .collect(),
    };
    assert!(matches!(
        Template::new(
            parameters(),
            body,
            native("return", &drops),
            HelperTemplateLimits {
                max_nodes: 4,
                ..LIMITS
            }
        )
        .unwrap_err(),
        HelperTemplateError::Body(HelperHygieneError::Structure(StructureError::NodeLimit))
    ));
    assert_eq!(drops.borrow().len(), 2);
    assert!(matches!(
        HelperTemplate::new(
            parameters(),
            choose(local("n")),
            (),
            HelperTemplateLimits {
                max_depth: 0,
                ..LIMITS
            }
        )
        .unwrap_err(),
        HelperTemplateError::Body(HelperHygieneError::Structure(StructureError::DepthLimit))
    ));
}

#[test]
fn debug_and_factory_error_never_dump_private_names_literals_or_native_metadata() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = Template::new(
        vec![("private_parameter".into(), ScalarType::String)],
        Node::Literal {
            value: ScalarValue::String("private body text".into()),
        },
        native("private return", &drops),
        LIMITS,
    )
    .unwrap();
    let debug = format!("{template:?}");
    assert!(debug.contains("parameters: 1"));
    assert!(!debug.contains("private"));
    let error =
        rejected(template.materialize(LIMITS, |_| Err(PrivateError("private factory error"))));
    assert!(!format!("{error:?} {error}").contains("private"));
    let HelperTemplateError::Factory(PrivateError(payload)) = error else {
        panic!()
    };
    assert_eq!(payload, "private factory error");
}

#[test]
fn narrower_call_limits_reject_before_the_factory_even_for_cold_bindings() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = template(bind("x", local("x")), &drops);
    for limits in [
        HelperTemplateLimits {
            max_nodes: 2,
            ..LIMITS
        },
        HelperTemplateLimits {
            max_depth: 0,
            ..LIMITS
        },
        HelperTemplateLimits {
            max_bindings: 1,
            ..LIMITS
        },
        HelperTemplateLimits {
            max_parameters: 0,
            ..LIMITS
        },
    ] {
        let calls = Cell::new(0);
        let result = template.materialize(limits, |_| {
            calls.set(calls.get() + 1);
            Ok::<_, PrivateError>(integer(0))
        });
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
    }
    assert!(drops.borrow().is_empty());
}

#[test]
fn factory_receives_exact_original_borrow_once_and_can_move_gui_local_slots() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = template(
        Node::Host {
            effect: Box::new(native("cached", &drops)),
        },
        &drops,
    );
    let effect = Box::new(native("materialized", &drops));
    let effect_box = effect.as_ref() as *const Native;
    let native_buffer = effect.bytes.as_ptr();
    let calls = Cell::new(0);
    let result = template
        .materialize(LIMITS, |source| {
            calls.set(calls.get() + 1);
            assert!(std::ptr::eq(source, template.body()));
            Ok::<_, PrivateError>(Node::Host { effect })
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    let Node::Host { effect } = &result else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const Native, effect_box);
    assert_eq!(effect.bytes.as_ptr(), native_buffer);
    assert!(drops.borrow().is_empty());
    drop(result);
    assert_eq!(*drops.borrow(), ["materialized"]);
    assert!(matches!(template.body(), Node::Host { effect } if effect.id == "cached"));
}

#[test]
fn returned_free_locals_and_parameter_shadowing_fail_without_partial_output() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = template(choose(local("n")), &drops);
    assert!(matches!(
        rejected(materialize(&template, choose(local("missing")))),
        HelperTemplateError::Body(HelperHygieneError::Capture { group: false })
    ));
    assert!(matches!(
        rejected(materialize(&template, choose(bind("n", local("n"))))),
        HelperTemplateError::Body(HelperHygieneError::ShadowedBinding)
    ));
    assert_eq!(template.shape(), HelperTemplateShape { nodes: 4, depth: 1 });
    assert!(drops.borrow().is_empty());
}

#[test]
fn smaller_or_differently_nested_output_cannot_change_reserved_physical_shape() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = template(choose(local("n")), &drops);
    let error = rejected(materialize(&template, integer(0)));
    assert!(matches!(
        error,
        HelperTemplateError::ShapeChanged {
            expected: HelperTemplateShape { nodes: 4, depth: 1 },
            actual: HelperTemplateShape { nodes: 1, depth: 0 }
        }
    ));
    let output = Node::Unary {
        operator: UnaryOperator::ToString,
        value: Box::new(Node::Unary {
            operator: UnaryOperator::ToString,
            value: Box::new(Node::Unary {
                operator: UnaryOperator::ToString,
                value: Box::new(integer(0)),
            }),
        }),
    };
    assert!(matches!(
        rejected(materialize(&template, output)),
        HelperTemplateError::ShapeChanged {
            expected: HelperTemplateShape { nodes: 4, depth: 1 },
            actual: HelperTemplateShape { nodes: 4, depth: 3 }
        }
    ));
}

#[test]
fn expanded_factory_output_is_bounded_and_its_native_slots_drop_once() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = template(integer(0), &drops);
    let output = Node::Group {
        group_kind: GroupKind::Parallel,
        branches: (0..64)
            .map(|i| ComputedBranch {
                name: format!("b{i}"),
                value: Node::Host {
                    effect: Box::new(native("output", &drops)),
                },
                result_type: native("tag", &drops),
            })
            .collect(),
    };
    assert!(matches!(
        rejected(template.materialize(
            HelperTemplateLimits {
                max_nodes: 4,
                ..LIMITS
            },
            |_| Ok::<_, PrivateError>(output)
        )),
        HelperTemplateError::Body(HelperHygieneError::Structure(StructureError::NodeLimit))
    ));
    assert_eq!(drops.borrow().len(), 128);
    assert_eq!(template.shape(), HelperTemplateShape { nodes: 1, depth: 0 });
}

#[test]
fn factory_error_and_unwind_preserve_cache_without_retry_or_native_side_effect_rollback() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let template = template(
        Node::Host {
            effect: Box::new(native("cached", &drops)),
        },
        &drops,
    );
    let calls = Cell::new(0);
    let result = template.materialize(LIMITS, |_| {
        calls.set(calls.get() + 1);
        let _partial = native("failed partial", &drops);
        Err::<Node, _>(PrivateError("private failure"))
    });
    assert!(matches!(rejected(result), HelperTemplateError::Factory(_)));
    assert_eq!(calls.get(), 1);
    let result = catch_unwind(AssertUnwindSafe(|| {
        template.materialize(LIMITS, |_| -> Result<Node, PrivateError> {
            calls.set(calls.get() + 1);
            let _partial = native("unwound partial", &drops);
            panic!("trusted factory unwind")
        })
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
    assert_eq!(*drops.borrow(), ["failed partial", "unwound partial"]);
    assert!(matches!(template.body(), Node::Host { effect } if effect.id == "cached"));
}

#[test]
fn cached_opaque_host_call_group_and_result_vectors_are_owned_without_rebuilding() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let arguments = vec![ComputedArgument {
        name: "input".into(),
        value: local("n"),
    }];
    let argument_buffer = arguments.as_ptr();
    let branches = vec![
        ComputedBranch {
            name: "one".into(),
            value: Node::Call {
                operation: native("operation", &drops),
                arguments,
            },
            result_type: native("tag one", &drops),
        },
        ComputedBranch {
            name: "two".into(),
            value: Node::Host {
                effect: Box::new(native("effect", &drops)),
            },
            result_type: native("tag two", &drops),
        },
    ];
    let branch_buffer = branches.as_ptr();
    let template = template(
        Node::Group {
            group_kind: GroupKind::Sequence,
            branches,
        },
        &drops,
    );
    assert_eq!(template.shape(), HelperTemplateShape { nodes: 4, depth: 2 });
    let (_, body, result) = template.into_parts();
    let Node::Group { branches, .. } = &body else {
        panic!()
    };
    assert_eq!(branches.as_ptr(), branch_buffer);
    let Node::Call { arguments, .. } = &branches[0].value else {
        panic!()
    };
    assert_eq!(arguments.as_ptr(), argument_buffer);
    assert!(drops.borrow().is_empty());
    drop((body, result));
    assert_eq!(drops.borrow().len(), 5);
}

struct NoHost;
impl PureTypeEnvironment<Native, Native> for NoHost {
    type Result = ();
    fn field_type(&self, _: &(), _: &Native) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &Native) -> Option<()> {
        None
    }
}

#[test]
fn equal_shape_does_not_certify_literal_operator_or_declared_return_type_identity() {
    let template =
        HelperTemplate::new(parameters(), local("n"), ScalarType::Integer, LIMITS).unwrap();
    let output = template
        .materialize(LIMITS, |_| {
            Ok::<_, PrivateError>(Node::Literal {
                value: ScalarValue::Boolean(false),
            })
        })
        .unwrap();
    assert_eq!(*template.result_type(), ScalarType::Integer);
    assert_eq!(
        infer_pure_type(&output, &[], &NoHost, TYPING),
        Ok(PureType::Scalar(ScalarType::Boolean))
    );
    let output = template
        .materialize(LIMITS, |_| {
            Ok::<_, PrivateError>(Node::Literal {
                value: ScalarValue::String("x".repeat(4097)),
            })
        })
        .unwrap();
    assert_eq!(
        infer_pure_type(&output, &[], &NoHost, TYPING),
        Err(PureTypeError::UnboundedLiteral)
    );
}

struct Device;
impl PureTypeEnvironment<u8, u32> for Device {
    type Result = ();
    fn field_type(&self, _: &(), _: &u8) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &u32) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<u8, u32> for Device {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &u8) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), ()> {
        Err(())
    }
}
fn eval_device(node: &Computation<u8, u32, (), ()>, fuel: &mut Fuel) -> ScalarValue {
    let mut values = Vec::new();
    let mut scope = ScopeFrame::new(&mut values);
    let result = evaluate_pure_in_scope(node, &mut scope, &Device, fuel, PURE).unwrap();
    assert!(scope.is_empty());
    let PureValue::Scalar(value) = result else {
        panic!()
    };
    value
}

#[test]
fn repeated_explicit_materialization_matches_original_value_and_exact_interpreter_fuel() {
    type DeviceNode = Computation<u8, u32, (), ()>;
    let body = DeviceNode::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(DeviceNode::Local { name: "n".into() }),
        right: Box::new(DeviceNode::Literal {
            value: ScalarValue::Integer(1),
        }),
    };
    let template = HelperTemplate::new(parameters(), body, ScalarType::Integer, LIMITS).unwrap();
    let original = DeviceNode::Bind {
        name: "n".into(),
        value: Box::new(DeviceNode::Literal {
            value: ScalarValue::Integer(41),
        }),
        body: Box::new(template.body().clone()),
    };
    let calls = Cell::new(0);
    for _ in 0..3 {
        let body = template
            .materialize(LIMITS, |body| {
                calls.set(calls.get() + 1);
                Ok::<_, ()>(body.clone())
            })
            .unwrap();
        let body = hygienic_helper_body(
            body,
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
        let wrapped = bind_helper_arguments(
            body,
            vec![HelperBinding {
                name: "_p".into(),
                value: DeviceNode::Literal {
                    value: ScalarValue::Integer(41),
                },
            }],
            HelperBindingLimits {
                max_nodes: 128,
                max_depth: 16,
                max_parameters: 8,
            },
        )
        .unwrap();
        let mut before = Fuel::new(100);
        let mut after = Fuel::new(100);
        assert!(matches!(
            eval_device(&original, &mut before),
            ScalarValue::Integer(42)
        ));
        assert!(matches!(
            eval_device(&wrapped, &mut after),
            ScalarValue::Integer(42)
        ));
        assert_eq!(before.remaining(), after.remaining());
    }
    assert_eq!(calls.get(), 3);
}

#[test]
fn parsed_helper_pipeline_owns_materializes_and_prepares_an_unrelated_native_call() {
    type DeviceNode = Computation<u8, u32, (), ()>;
    let tree = leselang_syntax::parse(
        "fn work(n: integer) = add(left: n, right: 1)\nfn launch() = work(n: 41)",
    );
    assert!(tree.diagnostics.is_empty());
    let mut order = helper_dependency_order(
        &[tree.function.as_ref().unwrap()],
        "launch",
        HelperDependencyLimits {
            max_helpers: 1,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
    )
    .unwrap();
    let helper = order.next().unwrap().unwrap();
    let signature = helper_parameters(helper, 8).unwrap();
    let source = SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    };
    let body = lower_scalar_source_with_scope(
        &helper.body,
        ScalarSourceLimits {
            source,
            max_bindings: 16,
        },
        &[(signature[0].name, signature[0].domain)],
        |_, _| -> Result<(DeviceNode, Option<ScalarType>), ()> { Err(()) },
    )
    .unwrap()
    .0;
    let template = HelperTemplate::new(
        signature
            .iter()
            .map(|parameter| (parameter.name.into(), parameter.domain))
            .collect(),
        body,
        ScalarType::Integer,
        LIMITS,
    )
    .unwrap();
    let args = lower_helper_arguments(
        &tree.helpers[0].body,
        HelperSignature {
            name: &helper.name,
            parameters: &signature,
        },
        HelperSourceLimits {
            source,
            max_parameters: 8,
        },
        |argument| {
            lower_scalar_source_with_scope(
                &argument.value,
                ScalarSourceLimits {
                    source,
                    max_bindings: 16,
                },
                &[],
                |_, _| -> Result<(DeviceNode, Option<ScalarType>), ()> { Err(()) },
            )
            .map(|(value, ty)| (value, Some(ty)))
        },
    )
    .unwrap();
    let body = template
        .materialize(LIMITS, |original| Ok::<_, ()>(original.clone()))
        .unwrap();
    let body = hygienic_helper_body(
        body,
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
    let value = bind_helper_arguments(
        body,
        args.into_iter()
            .map(|argument| HelperBinding {
                name: "_p".into(),
                value: argument.value,
            })
            .collect(),
        HelperBindingLimits {
            max_nodes: 128,
            max_depth: 16,
            max_parameters: 8,
        },
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(&value, &[], &Device, TYPING),
        Ok(PureType::Scalar(*template.result_type()))
    );
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: (),
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
    let call: DeviceNode = DeviceNode::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value,
        }],
    };
    let host = CallEvaluationHost {
        catalog: &catalog,
        version: 3,
        granted: &[31],
        environment: &Device,
    };
    let mut values = Vec::new();
    let mut scope = ScopeFrame::new(&mut values);
    let prepared = prepare_call_in_scope(
        &call,
        &mut scope,
        &host,
        &mut Fuel::new(100),
        CallEvaluationLimits {
            pure: PURE,
            max_arguments: 1,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert!(matches!(
        prepared.arguments()[0].value,
        ScalarValue::Integer(42)
    ));
    assert!(scope.is_empty());
    assert!(order.next().is_none());
}

#[test]
fn body_parameter_types_and_source_expansion_weights_are_not_template_shape_certificates() {
    let body = Node::Loop {
        name: "state".into(),
        initial: Box::new(local("n")),
        condition: Box::new(Node::Literal {
            value: ScalarValue::Boolean(false),
        }),
        next: Box::new(local("state")),
        limit: 1025,
    };
    let template = HelperTemplate::new(parameters(), body, ScalarType::Integer, LIMITS).unwrap();
    assert_eq!(
        infer_pure_type(
            template.body(),
            &[("n", PureType::Scalar(ScalarType::Integer))],
            &NoHost,
            TYPING
        ),
        Err(PureTypeError::LoopLimit)
    );
    let body = Node::Literal {
        value: ScalarValue::StringList(StringListValue(vec![String::new(); 64])),
    };
    let template = HelperTemplate::new(vec![], body, ScalarType::StringList, LIMITS).unwrap();
    assert_eq!(template.shape(), HelperTemplateShape { nodes: 1, depth: 0 });
}

#[test]
fn reference_repeated_nested_templates_keep_wire_diagnostics_and_cold_authority() {
    let source = "fn a(n: integer) = b(n: n)\nfn b(n: integer) = n\nfn main() = ui.focus(node_id: to_string(value: a(n: 7)))";
    let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let expected = leselang_syntax::format(&leselang_syntax::parse("fn main() = ui.focus(node_id: to_string(value: bind(_lf1: 7, body: bind(_lf2: _lf1, body: _lf2))))")).unwrap();
    assert_eq!(canonical, expected);
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap())
            .unwrap()
    );
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let mut body = "n".to_owned();
    for i in 0..16 {
        body = format!("bind(v{i}: 0, body: {body})");
    }
    let source = format!("fn deep(n: integer) = {body}\nfn main() = deep(n: 0)");
    assert!(
        leselang_hir::lower(&leselang_syntax::parse(&source))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405" && error.span.is_some())
    );
}
