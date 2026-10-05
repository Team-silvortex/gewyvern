use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_bindings::{HelperBinding, HelperBindingLimits, bind_helper_arguments};
use leselang_hir::helper_declarations::{HelperDeclarationLimits, accept_helper_declarations};
use leselang_hir::helper_dependencies::{HelperDependencyLimits, helper_dependency_order};
use leselang_hir::helper_expansion::*;
use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_source::{
    HelperSignature, HelperSourceLimits, helper_parameters, lower_helper_arguments,
};
use leselang_hir::helper_templates::{HelperTemplate, HelperTemplateError, HelperTemplateLimits};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::*;
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::*;

const LIMITS: HelperExpansionLimits = HelperExpansionLimits {
    max_nodes: 128,
    max_depth: 16,
    max_parameters: 8,
};
const TEMPLATE: HelperTemplateLimits = HelperTemplateLimits {
    max_nodes: 128,
    max_depth: 16,
    max_bindings: 16,
    max_parameters: 8,
};
const LEAF: HelperExpansionCost = HelperExpansionCost { nodes: 1, depth: 0 };
type Node = Computation<(), (), (), ()>;

fn reserve(
    used: &mut usize,
    caller: usize,
    body: HelperExpansionCost,
    depths: &[usize],
    limits: HelperExpansionLimits,
) -> Result<(), HelperExpansionError<Infallible>> {
    reserve_helper_expansion(used, caller, body, depths.len(), limits, |index| {
        Ok(depths[index])
    })
}
fn integer(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}

#[test]
fn append_body_and_wrappers_without_recharging_call_or_operands() {
    let mut used = 19;
    reserve(
        &mut used,
        2,
        HelperExpansionCost { nodes: 3, depth: 1 },
        &[0, 1],
        LIMITS,
    )
    .unwrap();
    assert_eq!(used, 24);
}

#[test]
fn zero_parameters_and_depth_allow_only_an_unwrapped_leaf_without_observation() {
    let mut used = 0;
    reserve_helper_expansion(
        &mut used,
        0,
        LEAF,
        0,
        HelperExpansionLimits {
            max_nodes: 1,
            max_depth: 0,
            max_parameters: 0,
        },
        |_| -> Result<usize, Infallible> { panic!("no operands") },
    )
    .unwrap();
    assert_eq!(used, 1);
    let mut used = 0;
    assert!(matches!(
        reserve(
            &mut used,
            0,
            LEAF,
            &[],
            HelperExpansionLimits {
                max_nodes: 0,
                ..LIMITS
            }
        ),
        Err(HelperExpansionError::NodeLimit)
    ));
    assert_eq!(used, 0);
}

#[test]
fn inclusive_safety_ceilings_do_not_install_a_default_policy() {
    let mut used = 16_383;
    reserve(
        &mut used,
        64,
        LEAF,
        &[],
        HelperExpansionLimits {
            max_nodes: 16_384,
            max_depth: 64,
            max_parameters: 8,
        },
    )
    .unwrap();
    assert_eq!(used, 16_384);
    for limits in [
        HelperExpansionLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        HelperExpansionLimits {
            max_depth: 65,
            ..LIMITS
        },
        HelperExpansionLimits {
            max_parameters: 9,
            ..LIMITS
        },
    ] {
        let mut used = 7;
        assert!(matches!(
            reserve_helper_expansion(
                &mut used,
                usize::MAX,
                LEAF,
                0,
                limits,
                |_| -> Result<usize, Infallible> { panic!("cheap rejection first") }
            ),
            Err(HelperExpansionError::InvalidLimits)
        ));
        assert_eq!(used, 7);
    }
}

#[test]
fn unused_parameters_still_charge_wrappers_and_shift_the_body() {
    let mut used = 9;
    reserve(
        &mut used,
        0,
        LEAF,
        &[0; 8],
        HelperExpansionLimits {
            max_nodes: 18,
            max_depth: 8,
            ..LIMITS
        },
    )
    .unwrap();
    assert_eq!(used, 18);
    let mut used = 9;
    assert!(matches!(
        reserve(
            &mut used,
            1,
            LEAF,
            &[0; 8],
            HelperExpansionLimits {
                max_depth: 8,
                ..LIMITS
            }
        ),
        Err(HelperExpansionError::BodyDepthLimit)
    ));
    assert_eq!(used, 9);
}

#[test]
fn parameter_limit_precedes_empty_body_and_any_native_observation() {
    let mut used = 12;
    assert!(matches!(
        reserve_helper_expansion(
            &mut used,
            0,
            HelperExpansionCost { nodes: 0, depth: 0 },
            9,
            LIMITS,
            |_| -> Result<usize, Infallible> { panic!("unadmitted signature") }
        ),
        Err(HelperExpansionError::ParameterLimit)
    ));
    assert_eq!(used, 12);
}

#[test]
fn empty_body_is_not_a_zero_cost_certificate() {
    let mut used = 12;
    assert!(matches!(
        reserve_helper_expansion(
            &mut used,
            0,
            HelperExpansionCost { nodes: 0, depth: 0 },
            1,
            LIMITS,
            |_| -> Result<usize, Infallible> { panic!("missing root") }
        ),
        Err(HelperExpansionError::EmptyBody)
    ));
    assert_eq!(used, 12);
}

#[test]
fn node_rejection_precedes_shifted_depth_and_observation() {
    let mut used = 128;
    assert!(matches!(
        reserve_helper_expansion(
            &mut used,
            usize::MAX,
            HelperExpansionCost {
                nodes: 1,
                depth: usize::MAX
            },
            1,
            LIMITS,
            |_| -> Result<usize, Infallible> { panic!("node gate first") }
        ),
        Err(HelperExpansionError::NodeLimit)
    ));
    assert_eq!(used, 128);
}

#[test]
fn node_overflow_is_checked_without_refunding_previous_charges() {
    for (before, nodes, count) in [(usize::MAX, 1, 0), (1, usize::MAX, 0), (0, usize::MAX, 1)] {
        let mut used = before;
        assert!(matches!(
            reserve_helper_expansion(
                &mut used,
                0,
                HelperExpansionCost { nodes, depth: 0 },
                count,
                LIMITS,
                |_| -> Result<usize, Infallible> { panic!("overflow") }
            ),
            Err(HelperExpansionError::NodeLimit)
        ));
        assert_eq!(used, before);
    }
}

#[test]
fn body_depth_overflow_rejects_before_native_argument_work() {
    for (caller, depth, count) in [(usize::MAX, 1, 0), (1, usize::MAX, 0), (0, usize::MAX, 1)] {
        let mut used = 5;
        assert!(matches!(
            reserve_helper_expansion(
                &mut used,
                caller,
                HelperExpansionCost { nodes: 1, depth },
                count,
                LIMITS,
                |_| -> Result<usize, Infallible> { panic!("shifted body overflow") }
            ),
            Err(HelperExpansionError::BodyDepthLimit)
        ));
        assert_eq!(used, 5);
    }
}

#[test]
fn operand_depth_uses_its_declaration_position_plus_the_caller_prefix() {
    let mut used = 3;
    reserve(&mut used, 3, LEAF, &[12, 11], LIMITS).unwrap();
    assert_eq!(used, 6);
    let mut used = 3;
    assert!(matches!(
        reserve(&mut used, 3, LEAF, &[12, 12], LIMITS),
        Err(HelperExpansionError::ArgumentDepthLimit { parameter_index: 1 })
    ));
    assert_eq!(used, 3);
}

#[test]
fn operand_depth_overflow_is_not_saturated_into_success() {
    let mut used = 3;
    assert!(matches!(
        reserve(&mut used, 0, LEAF, &[usize::MAX], LIMITS),
        Err(HelperExpansionError::ArgumentDepthLimit { parameter_index: 0 })
    ));
    assert_eq!(used, 3);
}

#[test]
fn observers_run_once_in_declaration_order_and_stop_at_first_bad_depth() {
    let calls = RefCell::new(Vec::new());
    let mut used = 7;
    let result = reserve_helper_expansion(&mut used, 0, LEAF, 3, LIMITS, |index| {
        calls.borrow_mut().push(index);
        Ok::<_, Infallible>(if index == 1 { 16 } else { 0 })
    });
    assert!(matches!(
        result,
        Err(HelperExpansionError::ArgumentDepthLimit { parameter_index: 1 })
    ));
    assert_eq!(*calls.borrow(), [0, 1]);
    assert_eq!(used, 7);
}

struct PrivateError {
    payload: &'static str,
    drops: Rc<Cell<usize>>,
}
impl Drop for PrivateError {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn native_errors_move_once_and_never_enter_formatting_or_source_chains() {
    let drops = Rc::new(Cell::new(0));
    let calls = RefCell::new(Vec::new());
    let mut used = 13;
    let error = reserve_helper_expansion(&mut used, 0, LEAF, 3, LIMITS, |index| {
        calls.borrow_mut().push(index);
        if index == 1 {
            Err(PrivateError {
                payload: "private native payload",
                drops: drops.clone(),
            })
        } else {
            Ok(0)
        }
    })
    .unwrap_err();
    assert!(
        matches!(&error, HelperExpansionError::Observation { parameter_index: 1, error }
        if error.payload == "private native payload")
    );
    assert_eq!(
        format!("{error:?} {error}"),
        "native helper depth observation failed native helper depth observation failed"
    );
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(*calls.borrow(), [0, 1]);
    assert_eq!(used, 13);
    drop(error);
    assert_eq!(drops.get(), 1);
}

#[test]
fn native_unwind_preserves_counter_without_retry_or_side_effect_rollback() {
    let calls = RefCell::new(Vec::new());
    let mut used = 13;
    let result = catch_unwind(AssertUnwindSafe(|| {
        reserve_helper_expansion(
            &mut used,
            0,
            LEAF,
            3,
            LIMITS,
            |index| -> Result<usize, Infallible> {
                calls.borrow_mut().push(index);
                if index == 1 {
                    panic!("trusted native observation unwind")
                }
                Ok(0)
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(*calls.borrow(), [0, 1]);
    assert_eq!(used, 13);
}

#[test]
fn committed_reservations_survive_later_factory_failure_and_unwind() {
    let template = HelperTemplate::new(vec![], integer(0), (), TEMPLATE).unwrap();
    let mut used = 5;
    reserve(&mut used, 0, LEAF, &[], LIMITS).unwrap();
    let result = template.materialize(TEMPLATE, |_| Err::<Node, _>("native failure"));
    assert!(matches!(result, Err(HelperTemplateError::Factory(_))));
    assert_eq!(used, 6);
    reserve(&mut used, 0, LEAF, &[], LIMITS).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            template.materialize(TEMPLATE, |_| -> Result<Node, Infallible> {
                panic!("factory unwind")
            })
        }))
        .is_err()
    );
    assert_eq!(used, 7);
}

#[test]
fn committed_reservation_is_not_refunded_by_wrapper_rejection() {
    let mut used = 3;
    reserve(&mut used, 0, LEAF, &[0], LIMITS).unwrap();
    assert!(
        bind_helper_arguments(
            integer(0),
            vec![HelperBinding {
                name: "bad alias".into(),
                value: integer(1),
            }],
            HelperBindingLimits {
                max_nodes: 128,
                max_depth: 16,
                max_parameters: 8
            }
        )
        .is_err()
    );
    assert_eq!(used, 5);
}

#[test]
fn repeated_attempts_accumulate_and_failed_append_does_not_refund() {
    let mut used = 1;
    let limits = HelperExpansionLimits {
        max_nodes: 7,
        ..LIMITS
    };
    for expected in [3, 5, 7] {
        reserve(&mut used, 0, LEAF, &[0], limits).unwrap();
        assert_eq!(used, expected);
    }
    assert!(matches!(
        reserve(&mut used, 0, LEAF, &[0], limits),
        Err(HelperExpansionError::NodeLimit)
    ));
    assert_eq!(used, 7);
}

#[test]
fn independent_callers_share_no_budget_or_native_state() {
    let mut first = 127;
    let mut second = 0;
    reserve(&mut first, 0, LEAF, &[], LIMITS).unwrap();
    reserve(&mut second, 0, LEAF, &[0, 0], LIMITS).unwrap();
    assert_eq!((first, second), (128, 3));
}

struct Native {
    bytes: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn move_only_native_operands_are_borrowed_not_cloned_or_owned_by_reservation() {
    type NativeNode = Computation<Native, Native, Native, Native>;
    let drops = Rc::new(Cell::new(0));
    let native = Native {
        bytes: vec![7; 31],
        drops: drops.clone(),
    };
    let buffer = native.bytes.as_ptr();
    let value = Box::new(NativeNode::Literal {
        value: ScalarValue::Integer(1),
    });
    let child = value.as_ref() as *const NativeNode;
    let operands = vec![NativeNode::Field {
        value,
        field: native,
    }];
    let vector = operands.as_ptr();
    let mut used = 3;
    reserve_helper_expansion(&mut used, 0, LEAF, 1, LIMITS, |index| {
        assert!(std::ptr::eq(&operands[index], &operands[0]));
        Ok::<_, Infallible>(1)
    })
    .unwrap();
    assert_eq!(operands.as_ptr(), vector);
    let NativeNode::Field { value, field } = &operands[0] else {
        panic!()
    };
    assert_eq!(value.as_ref() as *const NativeNode, child);
    assert_eq!(field.bytes.as_ptr(), buffer);
    assert_eq!(drops.get(), 0);
    drop(operands);
    assert_eq!(drops.get(), 1);
}

#[test]
fn folded_and_opaque_source_weights_are_not_physical_template_shape() {
    let body: Node = Node::Literal {
        value: ScalarValue::StringList(StringListValue(vec![String::new(); 64])),
    };
    let template = HelperTemplate::new(vec![], body, (), TEMPLATE).unwrap();
    assert_eq!(template.shape().nodes, 1);
    assert_eq!(template.shape().depth, 0);
    let mut used = 1;
    reserve(
        &mut used,
        0,
        HelperExpansionCost {
            nodes: 65,
            depth: 1,
        },
        &[],
        LIMITS,
    )
    .unwrap();
    assert_eq!(used, 66);
    let opaque: Node = Node::Host {
        effect: Box::new(()),
    };
    let template = HelperTemplate::new(vec![], opaque, (), TEMPLATE).unwrap();
    assert_eq!(template.shape().nodes, 1);
    reserve(
        &mut used,
        0,
        HelperExpansionCost { nodes: 7, depth: 3 },
        &[],
        LIMITS,
    )
    .unwrap();
    assert_eq!(used, 73);
}

struct Host;
impl PureTypeEnvironment<(), ()> for Host {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &()) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<(), ()> for Host {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &()) -> Result<(), ()> {
        Err(())
    }
}

#[test]
fn supplied_metadata_cannot_certify_cold_types_or_actual_output_shape() {
    let mut used = 0;
    reserve(&mut used, 0, LEAF, &[], LIMITS).unwrap();
    let invalid = Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(integer(0)),
        otherwise: Box::new(Node::Local {
            name: "missing".into(),
        }),
    };
    assert!(
        infer_pure_type(
            &invalid,
            &[],
            &Host,
            TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16,
            }
        )
        .is_err()
    );
    assert!(
        bind_helper_arguments(
            invalid,
            vec![],
            HelperBindingLimits {
                max_nodes: 1,
                max_depth: 0,
                max_parameters: 0,
            }
        )
        .is_err()
    );
    assert_eq!(used, 1);
}

#[test]
fn valid_adapter_inputs_match_legacy_short_circuit_arithmetic_and_observer_order() {
    for before in [0usize, 1, 119, 127, 128, usize::MAX] {
        for nodes in [1usize, 3, 128, usize::MAX] {
            for caller in [0usize, 8, 16, usize::MAX] {
                for depth in [0usize, 1, 16, usize::MAX] {
                    for count in [0usize, 1, 8] {
                        let depths = [0usize, 16, usize::MAX, 0, 1, 0, 0, 0];
                        let mut legacy_calls = Vec::new();
                        let legacy_bad = before.saturating_add(nodes).saturating_add(count) > 128
                            || caller.saturating_add(depth).saturating_add(count) > 16
                            || (0..count).any(|index| {
                                legacy_calls.push(index);
                                caller
                                    .saturating_add(index + 1)
                                    .saturating_add(depths[index])
                                    > 16
                            });
                        let mut calls = Vec::new();
                        let mut used = before;
                        let result = reserve_helper_expansion(
                            &mut used,
                            caller,
                            HelperExpansionCost { nodes, depth },
                            count,
                            LIMITS,
                            |index| {
                                calls.push(index);
                                Ok::<_, Infallible>(depths[index])
                            },
                        );
                        assert_eq!(result.is_err(), legacy_bad);
                        assert_eq!(calls, legacy_calls);
                        assert_eq!(
                            used,
                            if legacy_bad {
                                before
                            } else {
                                before + nodes + count
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn parsed_declarations_to_reserved_templates_hygiene_and_exact_interpreter_fuel() {
    let tree = leselang_syntax::parse(
        "fn work(n: integer) = add(left: n, right: 1)\nfn launch() = work(n: 41)",
    );
    assert!(tree.diagnostics.is_empty());
    let input = tree
        .function
        .iter()
        .chain(tree.helpers.iter())
        .collect::<Vec<_>>();
    let admitted = accept_helper_declarations(
        &input,
        "launch",
        HelperDeclarationLimits {
            max_functions: 2,
            max_parameters: 8,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
        |_| Ok::<_, Infallible>(false),
    )
    .unwrap();
    let mut order = helper_dependency_order(
        admitted.helpers(),
        "launch",
        HelperDependencyLimits {
            max_helpers: 1,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
    )
    .unwrap();
    let helper = order.next().unwrap().unwrap();
    let parameters = helper_parameters(helper, 8).unwrap();
    let source = SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    };
    let scalar = ScalarSourceLimits {
        source,
        max_bindings: 16,
    };
    let body = lower_scalar_source_with_scope(
        &helper.body,
        scalar,
        &[(parameters[0].name, parameters[0].domain)],
        |_, _| -> Result<(Node, Option<ScalarType>), ()> { Err(()) },
    )
    .unwrap()
    .0;
    let template = HelperTemplate::new(
        vec![("n".into(), ScalarType::Integer)],
        body,
        ScalarType::Integer,
        TEMPLATE,
    )
    .unwrap();
    let operands = lower_helper_arguments(
        &admitted.entry().body,
        HelperSignature {
            name: &helper.name,
            parameters: &parameters,
        },
        HelperSourceLimits {
            source,
            max_parameters: 8,
        },
        |argument| {
            lower_scalar_source_with_scope(
                &argument.value,
                scalar,
                &[],
                |_, _| -> Result<(Node, Option<ScalarType>), ()> { Err(()) },
            )
            .map(|(value, ty)| (value, Some(ty)))
        },
    )
    .unwrap();
    let mut used = 2;
    reserve_helper_expansion(
        &mut used,
        0,
        HelperExpansionCost { nodes: 3, depth: 1 },
        parameters.len(),
        LIMITS,
        |index| {
            assert!(matches!(operands[index].value, Node::Literal { .. }));
            Ok::<_, Infallible>(0)
        },
    )
    .unwrap();
    assert_eq!(used, 6);
    let body = template
        .materialize(TEMPLATE, |body| Ok::<_, Infallible>(body.clone()))
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
        operands
            .into_iter()
            .map(|operand| HelperBinding {
                name: "_p".into(),
                value: operand.value,
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
        infer_pure_type(
            &wrapped,
            &[],
            &Host,
            TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16,
            }
        ),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let original = Node::Bind {
        name: "n".into(),
        value: Box::new(integer(41)),
        body: Box::new(template.body().clone()),
    };
    let mut before = Fuel::new(100);
    let mut after = Fuel::new(100);
    for (node, fuel) in [(&original, &mut before), (&wrapped, &mut after)] {
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let result = evaluate_pure_in_scope(
            node,
            &mut scope,
            &Host,
            fuel,
            PureEvaluationLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16,
            },
        )
        .unwrap();
        assert!(matches!(
            result,
            PureValue::Scalar(ScalarValue::Integer(42))
        ));
        assert!(scope.is_empty());
    }
    assert_eq!(before.remaining(), after.remaining());
    assert_eq!(used, 6);
}

#[test]
fn reference_wire_fresh_names_authority_and_expansion_diagnostics_are_unchanged() {
    let source = "fn work(a: string, b: string) = ui.focus(node_id: concat(left: a, right: b))\nfn main() = work(b: \"b\", a: \"a\")";
    let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let expected = leselang_syntax::format(&leselang_syntax::parse("fn main() = bind(_lf0: \"a\", body: bind(_lf1: \"b\", body: ui.focus(node_id: concat(left: _lf0, right: _lf1))))")).unwrap();
    assert_eq!(canonical, expected);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap())
            .unwrap()
    );
    let parameters = (0..8)
        .map(|i| format!("p{i}: integer"))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = (0..8)
        .map(|i| format!("p{i}: 0"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = format!("work({arguments})");
    for _ in 0..9 {
        body = format!("add(left: {body}, right: 0)");
    }
    let source = format!("fn work({parameters}) = add(left: p0, right: 1)\nfn main() = {body}");
    let tree = leselang_syntax::parse(&source);
    assert!(tree.diagnostics.is_empty());
    let errors = leselang_hir::lower(&tree).unwrap_err();
    assert_eq!(errors[0].code, "LSH1405");
    assert_eq!(
        errors[0].message,
        "expanded helper exceeds computation bounds"
    );
    assert_eq!(
        errors[0].span.unwrap().start,
        source.rfind("work(").unwrap()
    );
}
