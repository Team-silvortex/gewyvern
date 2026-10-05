use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_bindings::{HelperBinding, HelperBindingLimits, bind_helper_arguments};
use leselang_hir::helper_expansion::{
    HelperExpansionCost, HelperExpansionLimits, reserve_helper_expansion,
};
use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_source::{
    HelperSignature, HelperSourceLimits, helper_parameters, lower_helper_arguments,
};
use leselang_hir::helper_templates::{HelperTemplate, HelperTemplateLimits};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::*;
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::*;
use leselang_runtime_core::*;

const LIMITS: SourceCostLimits = SourceCostLimits {
    max_nodes: 128,
    max_depth: 16,
};
const ZERO: SourceCostExtra = SourceCostExtra { nodes: 0, depth: 0 };
type Node = Computation<u8, u32, u8, u64>;
fn literal(value: ScalarValue) -> Node {
    Node::Literal { value }
}
fn integer() -> Node {
    literal(ScalarValue::Integer(0))
}
fn host(id: u8) -> Node {
    Node::Host {
        effect: Box::new(id),
    }
}
fn unary(mut node: Node, count: usize) -> Node {
    for _ in 0..count {
        node = Node::Unary {
            operator: UnaryOperator::ToString,
            value: Box::new(node),
        };
    }
    node
}
fn cost(node: &Node) -> HelperExpansionCost {
    measure_source_cost(node, LIMITS, |_| Ok::<_, Infallible>(ZERO)).unwrap()
}

#[test]
fn folded_literal_weights_include_optional_none_but_not_empty_list_children() {
    for (value, nodes, depth) in [
        (ScalarValue::Integer(0), 1, 0),
        (ScalarValue::Boolean(false), 1, 0),
        (ScalarValue::None, 1, 0),
        (ScalarValue::String("private".into()), 1, 0),
        (ScalarValue::OptionalString(OptionalStringValue(None)), 2, 1),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some("private".into()))),
            2,
            1,
        ),
        (ScalarValue::StringList(StringListValue(vec![])), 1, 0),
        (
            ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
            3,
            1,
        ),
    ] {
        let extra = literal_source_extra(&value);
        assert_eq!((extra.nodes, extra.depth), (nodes - 1, depth));
        assert_eq!(cost(&literal(value)), HelperExpansionCost { nodes, depth });
    }
}

#[test]
fn folded_and_unfolded_constructor_costs_match_without_copying_buffers() {
    let items = vec![
        literal(ScalarValue::String("a".into())),
        literal(ScalarValue::String("b".into())),
    ];
    let unfolded = Node::Strings { items };
    let folded = literal(ScalarValue::StringList(StringListValue(vec![
        "a".into(),
        "b".into(),
    ])));
    assert_eq!(cost(&folded), cost(&unfolded));
    for input in [ScalarValue::None, ScalarValue::String("private".into())] {
        let result = match &input {
            ScalarValue::None => None,
            ScalarValue::String(value) => Some(value.clone()),
            _ => unreachable!(),
        };
        assert_eq!(
            cost(&literal(ScalarValue::OptionalString(OptionalStringValue(
                result
            )))),
            cost(&Node::Unary {
                operator: UnaryOperator::OptionalString,
                value: Box::new(literal(input))
            })
        );
    }
}

#[test]
fn every_control_call_group_projection_form_uses_original_language_children() {
    let cases = vec![
        (integer(), 1, 0),
        (
            Node::Strings {
                items: vec![integer()],
            },
            2,
            1,
        ),
        (Node::Local { name: "n".into() }, 1, 0),
        (
            Node::Field {
                value: Box::new(integer()),
                field: 1,
            },
            2,
            1,
        ),
        (
            Node::Member {
                group: "g".into(),
                name: "m".into(),
                operation: 17,
            },
            1,
            0,
        ),
        (
            Node::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(integer()),
                right: Box::new(integer()),
            },
            3,
            1,
        ),
        (unary(integer(), 1), 2, 1),
        (
            Node::Bind {
                name: "n".into(),
                value: Box::new(integer()),
                body: Box::new(integer()),
            },
            3,
            1,
        ),
        (
            Node::Loop {
                name: "n".into(),
                initial: Box::new(integer()),
                condition: Box::new(integer()),
                next: Box::new(integer()),
                limit: 0,
            },
            4,
            1,
        ),
        (
            Node::Fold {
                name: "n".into(),
                item: "i".into(),
                items: Box::new(integer()),
                initial: Box::new(integer()),
                next: Box::new(integer()),
                limit: 0,
            },
            4,
            1,
        ),
        (
            Node::Choose {
                when: Box::new(integer()),
                then: Box::new(integer()),
                otherwise: Box::new(integer()),
            },
            4,
            1,
        ),
        (
            Node::Recover {
                value: Box::new(integer()),
                fallback: Box::new(integer()),
            },
            3,
            1,
        ),
        (host(0), 1, 0),
        (
            Node::Call {
                operation: 17,
                arguments: vec![ComputedArgument {
                    name: "n".into(),
                    value: integer(),
                }],
            },
            2,
            1,
        ),
        (
            Node::Group {
                group_kind: GroupKind::Parallel,
                branches: vec![ComputedBranch {
                    name: "b".into(),
                    value: integer(),
                    result_type: 31,
                }],
            },
            2,
            1,
        ),
    ];
    assert_eq!(cases.len(), 15);
    for (node, nodes, depth) in cases {
        assert_eq!(cost(&node), HelperExpansionCost { nodes, depth });
    }
}

#[test]
fn zero_native_extras_are_explicit_and_zero_language_nodes_still_deny_roots() {
    let limits = SourceCostLimits {
        max_nodes: 1,
        max_depth: 0,
    };
    assert_eq!(
        measure_source_cost(&host(0), limits, |_| Ok::<_, Infallible>(ZERO)).unwrap(),
        HelperExpansionCost { nodes: 1, depth: 0 }
    );
    assert!(matches!(
        measure_source_cost(
            &host(0),
            SourceCostLimits {
                max_nodes: 0,
                ..limits
            },
            |_| -> Result<SourceCostExtra, Infallible> { panic!("no admitted root") }
        ),
        Err(SourceCostError::Structure(StructureError::NodeLimit))
    ));
}

#[test]
fn invalid_ceilings_reject_before_any_native_observer() {
    for limits in [
        SourceCostLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        SourceCostLimits {
            max_depth: 65,
            ..LIMITS
        },
    ] {
        assert!(matches!(
            measure_source_cost(
                &host(0),
                limits,
                |_| -> Result<SourceCostExtra, Infallible> { panic!("invalid policy") }
            ),
            Err(SourceCostError::InvalidLimits)
        ));
    }
    assert_eq!(
        measure_source_cost(
            &unary(integer(), 64),
            SourceCostLimits {
                max_nodes: 16_384,
                max_depth: 64
            },
            |_| Ok::<_, Infallible>(ZERO)
        )
        .unwrap(),
        HelperExpansionCost {
            nodes: 65,
            depth: 64
        }
    );
}

#[test]
fn complete_cold_language_tree_precedes_native_observation() {
    let node = Node::Choose {
        when: Box::new(integer()),
        then: Box::new(unary(integer(), 16)),
        otherwise: Box::new(host(0)),
    };
    let calls = Cell::new(0);
    let error = measure_source_cost(&node, LIMITS, |_| {
        calls.set(calls.get() + 1);
        Ok::<_, Infallible>(ZERO)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        SourceCostError::Structure(StructureError::DepthLimit)
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn pending_frontier_is_bounded_before_growing_or_observing_cold_hosts() {
    let node = Node::Group {
        group_kind: GroupKind::Parallel,
        branches: (0..256)
            .map(|i| ComputedBranch {
                name: format!("b{i}"),
                value: host(0),
                result_type: 0,
            })
            .collect(),
    };
    let before = serde_json::to_vec(&node).unwrap();
    assert!(matches!(
        measure_source_cost(&node, LIMITS, |_| -> Result<SourceCostExtra, Infallible> {
            panic!("frontier rejection")
        }),
        Err(SourceCostError::Structure(StructureError::NodeLimit))
    ));
    assert_eq!(before, serde_json::to_vec(&node).unwrap());
}

#[test]
fn folded_cold_constructor_bounds_precede_native_observation() {
    let node = Node::Choose {
        when: Box::new(integer()),
        then: Box::new(literal(ScalarValue::StringList(StringListValue(
            vec![String::new(); 128],
        )))),
        otherwise: Box::new(host(0)),
    };
    assert!(matches!(
        measure_source_cost(&node, LIMITS, |_| -> Result<SourceCostExtra, Infallible> {
            panic!("folded budget first")
        }),
        Err(SourceCostError::Structure(StructureError::NodeLimit))
    ));
}

#[test]
fn native_sites_use_rightmost_first_dfs_and_relative_language_root_depth() {
    let node = Node::Choose {
        when: Box::new(integer()),
        then: Box::new(host(1)),
        otherwise: Box::new(Node::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(integer()),
            right: Box::new(host(2)),
        }),
    };
    let calls = RefCell::new(Vec::new());
    let result = measure_source_cost(&node, LIMITS, |effect| {
        calls.borrow_mut().push(*effect);
        Ok::<_, Infallible>(SourceCostExtra {
            nodes: *effect as usize,
            depth: *effect as usize,
        })
    })
    .unwrap();
    assert_eq!(*calls.borrow(), [2, 1]);
    assert_eq!(result, HelperExpansionCost { nodes: 9, depth: 4 });
}

#[test]
fn zero_iteration_loops_folds_and_recovery_still_include_all_cold_children() {
    for node in [
        Node::Loop {
            name: "n".into(),
            initial: Box::new(integer()),
            condition: Box::new(host(1)),
            next: Box::new(host(2)),
            limit: 0,
        },
        Node::Fold {
            name: "n".into(),
            item: "i".into(),
            items: Box::new(integer()),
            initial: Box::new(host(1)),
            next: Box::new(host(2)),
            limit: 0,
        },
        Node::Recover {
            value: Box::new(host(1)),
            fallback: Box::new(host(2)),
        },
    ] {
        let calls = RefCell::new(Vec::new());
        measure_source_cost(&node, LIMITS, |effect| {
            calls.borrow_mut().push(*effect);
            Ok::<_, Infallible>(ZERO)
        })
        .unwrap();
        assert_eq!(*calls.borrow(), [2, 1]);
    }
}

#[test]
fn repeated_occurrences_are_not_deduplicated_by_native_identity() {
    let node = Node::Recover {
        value: Box::new(host(1)),
        fallback: Box::new(host(1)),
    };
    let calls = Cell::new(0);
    assert_eq!(
        measure_source_cost(&node, LIMITS, |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(SourceCostExtra { nodes: 1, depth: 1 })
        })
        .unwrap(),
        HelperExpansionCost { nodes: 5, depth: 2 }
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn native_node_overflow_returns_no_partial_cost_or_retry() {
    let calls = Cell::new(0);
    assert!(matches!(
        measure_source_cost(&host(0), LIMITS, |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(SourceCostExtra {
                nodes: usize::MAX,
                depth: 0,
            })
        }),
        Err(SourceCostError::Structure(StructureError::NodeLimit))
    ));
    assert_eq!(calls.get(), 1);
}

#[test]
fn native_depth_overflow_and_depth_before_node_priority_are_checked() {
    let node = unary(host(0), 1);
    assert!(matches!(
        measure_source_cost(&node, LIMITS, |_| Ok::<_, Infallible>(SourceCostExtra {
            nodes: usize::MAX,
            depth: usize::MAX,
        })),
        Err(SourceCostError::Structure(StructureError::DepthLimit))
    ));
    assert!(matches!(
        measure_source_cost(&node, LIMITS, |_| Ok::<_, Infallible>(SourceCostExtra {
            nodes: 0,
            depth: 16,
        })),
        Err(SourceCostError::Structure(StructureError::DepthLimit))
    ));
}

#[test]
fn native_cost_limits_are_inclusive_and_failure_stops_later_sites() {
    let node = Node::Recover {
        value: Box::new(host(1)),
        fallback: Box::new(host(2)),
    };
    let limits = SourceCostLimits {
        max_nodes: 5,
        max_depth: 2,
    };
    assert_eq!(
        measure_source_cost(&node, limits, |_| Ok::<_, Infallible>(SourceCostExtra {
            nodes: 1,
            depth: 1,
        }))
        .unwrap(),
        HelperExpansionCost { nodes: 5, depth: 2 }
    );
    let calls = RefCell::new(Vec::new());
    assert!(matches!(
        measure_source_cost(&node, limits, |id| {
            calls.borrow_mut().push(*id);
            Ok::<_, Infallible>(SourceCostExtra { nodes: 3, depth: 1 })
        }),
        Err(SourceCostError::Structure(StructureError::NodeLimit))
    ));
    assert_eq!(*calls.borrow(), [2]);
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
fn native_errors_move_once_without_formatter_bounds_or_payload_source_chains() {
    let drops = Rc::new(Cell::new(0));
    let node = Node::Recover {
        value: Box::new(host(1)),
        fallback: Box::new(host(2)),
    };
    let calls = RefCell::new(Vec::new());
    let error = measure_source_cost(&node, LIMITS, |id| {
        calls.borrow_mut().push(*id);
        if *id == 1 {
            Err(PrivateError {
                payload: "private native payload",
                drops: drops.clone(),
            })
        } else {
            Ok(ZERO)
        }
    })
    .unwrap_err();
    assert!(
        matches!(&error, SourceCostError::Observation { host_index: 1, error } if error.payload == "private native payload")
    );
    assert_eq!(
        format!("{error:?} {error}"),
        "native source cost observation failed native source cost observation failed"
    );
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(*calls.borrow(), [2, 1]);
    drop(error);
    assert_eq!(drops.get(), 1);
}

#[test]
fn native_unwind_does_not_retry_rollback_effects_or_mutate_borrowed_ir() {
    let node = Node::Recover {
        value: Box::new(host(1)),
        fallback: Box::new(host(2)),
    };
    let before = serde_json::to_vec(&node).unwrap();
    let calls = RefCell::new(Vec::new());
    assert!(
        catch_unwind(AssertUnwindSafe(|| measure_source_cost(
            &node,
            LIMITS,
            |id| -> Result<SourceCostExtra, Infallible> {
                calls.borrow_mut().push(*id);
                if *id == 1 {
                    panic!("trusted observation unwind")
                }
                Ok(ZERO)
            }
        )))
        .is_err()
    );
    assert_eq!(*calls.borrow(), [2, 1]);
    assert_eq!(before, serde_json::to_vec(&node).unwrap());
}

struct Native {
    id: &'static str,
    bytes: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn move_only_native_slots_original_boxes_vectors_and_literal_buffers_are_borrowed() {
    type NativeNode = Computation<Native, Native, Native, Native>;
    let drops = Rc::new(Cell::new(0));
    let native = |id| Native {
        id,
        bytes: vec![7; 31],
        drops: drops.clone(),
    };
    let effect = Box::new(native("effect"));
    let effect_box = effect.as_ref() as *const Native;
    let effect_buffer = effect.bytes.as_ptr();
    let items = vec!["private literal".to_owned()];
    let list_vector = items.as_ptr();
    let text_buffer = items[0].as_ptr();
    let branches = vec![
        ComputedBranch {
            name: "field".into(),
            value: NativeNode::Field {
                value: Box::new(NativeNode::Literal {
                    value: ScalarValue::StringList(StringListValue(items)),
                }),
                field: native("field"),
            },
            result_type: native("field return"),
        },
        ComputedBranch {
            name: "call".into(),
            value: NativeNode::Call {
                operation: native("operation"),
                arguments: vec![],
            },
            result_type: native("call return"),
        },
        ComputedBranch {
            name: "host".into(),
            value: NativeNode::Host { effect },
            result_type: native("host return"),
        },
    ];
    let branch_vector = branches.as_ptr();
    let node = NativeNode::Group {
        group_kind: GroupKind::Parallel,
        branches,
    };
    measure_source_cost(&node, LIMITS, |effect| {
        assert_eq!(effect.id, "effect");
        assert_eq!(effect.bytes.as_ptr(), effect_buffer);
        assert!(std::ptr::eq(effect, effect_box));
        Ok::<_, Infallible>(SourceCostExtra { nodes: 1, depth: 1 })
    })
    .unwrap();
    let NativeNode::Group { branches, .. } = &node else {
        panic!()
    };
    assert_eq!(branches.as_ptr(), branch_vector);
    let NativeNode::Field { value, .. } = &branches[0].value else {
        panic!()
    };
    let NativeNode::Literal {
        value: ScalarValue::StringList(items),
    } = value.as_ref()
    else {
        panic!()
    };
    assert_eq!(items.0.as_ptr(), list_vector);
    assert_eq!(items.0[0].as_ptr(), text_buffer);
    assert_eq!(drops.get(), 0);
    drop(node);
    assert_eq!(drops.get(), 6);
}

#[test]
fn measurement_does_not_certify_literal_bytes_lexical_types_or_native_metadata() {
    let invalid = Node::Choose {
        when: Box::new(literal(ScalarValue::Boolean(true))),
        then: Box::new(integer()),
        otherwise: Box::new(Node::Local {
            name: "missing".into(),
        }),
    };
    assert_eq!(cost(&invalid), HelperExpansionCost { nodes: 4, depth: 1 });
    assert!(
        infer_pure_type(
            &invalid,
            &[],
            &Device,
            TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16
            }
        )
        .is_err()
    );
    let unbounded = literal(ScalarValue::String("private".repeat(1000)));
    assert_eq!(cost(&unbounded), HelperExpansionCost { nodes: 1, depth: 0 });
    assert_eq!(
        infer_pure_type(
            &unbounded,
            &[],
            &Device,
            TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16
            }
        ),
        Err(PureTypeError::UnboundedLiteral)
    );
    assert_eq!(cost(&host(0)), HelperExpansionCost { nodes: 1, depth: 0 });
}

#[test]
fn observed_cost_is_not_a_lasting_certificate_or_shared_global_meter() {
    let mut node = integer();
    let before = cost(&node);
    node = unary(node, 2);
    assert_eq!(before, HelperExpansionCost { nodes: 1, depth: 0 });
    assert_eq!(cost(&node), HelperExpansionCost { nodes: 3, depth: 2 });
    assert_eq!(cost(&integer()), before);
}

fn legacy(node: &Node) -> HelperExpansionCost {
    let mut pending = vec![(node, 0)];
    let mut nodes = 0;
    let mut depth = 0;
    while let Some((node, level)) = pending.pop() {
        let extra = match node {
            Node::Literal { value } => literal_source_extra(value),
            _ => ZERO,
        };
        nodes += 1 + extra.nodes;
        depth = depth.max(level + extra.depth);
        if let Node::Host { effect } = node {
            nodes += **effect as usize;
            depth = depth.max(level + **effect as usize);
        }
        pending.extend(node.children().map(|child| (child, level + 1)));
    }
    HelperExpansionCost { nodes, depth }
}

#[test]
fn bounded_adapter_inputs_match_legacy_costs_and_independent_inclusive_limits() {
    for depth in 0..8 {
        for size in [0, 1, 8, 64] {
            let node = Node::Choose {
                when: Box::new(integer()),
                then: Box::new(unary(
                    literal(ScalarValue::StringList(StringListValue(vec![
                        String::new();
                        size
                    ]))),
                    depth,
                )),
                otherwise: Box::new(unary(host(2), depth)),
            };
            let expected = legacy(&node);
            for nodes in [expected.nodes - 1, expected.nodes, 128] {
                for bound in [expected.depth - 1, expected.depth, 16] {
                    let result = measure_source_cost(
                        &node,
                        SourceCostLimits {
                            max_nodes: nodes,
                            max_depth: bound,
                        },
                        |id| {
                            Ok::<_, Infallible>(SourceCostExtra {
                                nodes: *id as usize,
                                depth: *id as usize,
                            })
                        },
                    );
                    assert_eq!(
                        result.is_ok(),
                        nodes >= expected.nodes && bound >= expected.depth
                    );
                    if let Ok(actual) = result {
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }
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

#[test]
fn parsed_folded_operands_compose_with_reservation_templates_hygiene_and_exact_fuel() {
    let tree = leselang_syntax::parse(
        "fn work(n: optional_string) = value_or(left: n, right: \"missing\")\nfn launch() = work(n: optional_string(value: \"ready\"))",
    );
    assert!(tree.diagnostics.is_empty());
    let declarations = tree
        .function
        .iter()
        .chain(tree.helpers.iter())
        .collect::<Vec<_>>();
    let helper = declarations.iter().find(|f| f.name == "work").unwrap();
    let entry = declarations.iter().find(|f| f.name == "launch").unwrap();
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
    let body_cost = cost(&body);
    let template = HelperTemplate::new(
        vec![("n".into(), ScalarType::OptionalString)],
        body,
        ScalarType::String,
        HelperTemplateLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
            max_parameters: 8,
        },
    )
    .unwrap();
    let operands = lower_helper_arguments(
        &entry.body,
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
    assert!(matches!(operands[0].value, Node::Literal { .. }));
    let mut used = 1 + cost(&operands[0].value).nodes;
    assert_eq!(used, 3);
    reserve_helper_expansion(
        &mut used,
        0,
        body_cost,
        parameters.len(),
        HelperExpansionLimits {
            max_nodes: 128,
            max_depth: 16,
            max_parameters: 8,
        },
        |index| Ok::<_, Infallible>(cost(&operands[index].value).depth),
    )
    .unwrap();
    assert_eq!(used, 7);
    let limits = HelperTemplateLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 16,
        max_parameters: 8,
    };
    let body = template
        .materialize(limits, |body| Ok::<_, Infallible>(body.clone()))
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
    let argument = operands.into_iter().next().unwrap().value;
    let original = Node::Bind {
        name: "n".into(),
        value: Box::new(argument.clone()),
        body: Box::new(template.body().clone()),
    };
    let wrapped = bind_helper_arguments(
        body,
        vec![HelperBinding {
            name: "_p".into(),
            value: argument,
        }],
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
            &Device,
            TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16
            }
        ),
        Ok(PureType::Scalar(ScalarType::String))
    );
    let mut before = Fuel::new(100);
    let mut after = Fuel::new(100);
    for (node, fuel) in [(&original, &mut before), (&wrapped, &mut after)] {
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let result = evaluate_pure_in_scope(
            node,
            &mut scope,
            &Device,
            fuel,
            PureEvaluationLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16,
            },
        )
        .unwrap();
        assert!(
            matches!(result, PureValue::Scalar(ScalarValue::String(value)) if value == "ready")
        );
        assert!(scope.is_empty());
    }
    assert_eq!(before.remaining(), after.remaining());
    assert_eq!(used, 7);
}

#[test]
fn reference_helper_returns_wire_capabilities_and_cold_repeat_bounds_are_unchanged() {
    for source in [
        "fn work(n: string) = bind(r: ui.focus(node_id: n), body: field(value: r, name: \"node_id\"))\nfn main() = bind(answer: work(n: \"a\"), body: ui.focus(node_id: answer))",
        "fn work() = bind(r: ui.focus(node_id: \"a\"), body: optional_string(value: none))\nfn main() = bind(answer: choose(when: true, then: work(), otherwise: optional_string(value: none)), body: value_or(left: answer, right: \"fallback\"))",
        "fn work() = bind(r: ui.focus(node_id: \"a\"), body: strings(a: \"x\"))\nfn main() = bind(answer: work(), body: join(left: answer, right: \";\"))",
    ] {
        let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap())
                .unwrap()
        );
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        assert!(
            leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
                .is_err()
        );
    }
    let mut value = "\"a\"".to_owned();
    for _ in 0..3 {
        value = format!("concat(left: {value}, right: {value})");
    }
    for (count, accepted) in [(63, true), (64, false)] {
        let source =
            format!("fn main() = repeat(times: {count}, body: ui.focus(node_id: {value}))");
        let tree = leselang_syntax::parse(&source);
        assert!(tree.diagnostics.is_empty());
        let result = leselang_hir::lower(&tree);
        assert_eq!(result.is_ok(), accepted);
        if let Err(errors) = result {
            assert_eq!(errors[0].code, "LSH1405");
            assert_eq!(
                errors[0].message,
                "repeated computation exceeds the expanded node limit"
            );
            assert!(errors[0].span.is_some());
        }
    }
}
