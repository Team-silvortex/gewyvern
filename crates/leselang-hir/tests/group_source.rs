use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::group_source::{
    GroupSourceError, GroupSourceLimits, GroupSourcePhase, GroupSourceResult,
    lower_flat_group_source,
};
use leselang_hir::ir::{Computation, ComputedArgument, GroupKind};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{ScalarValue, ScopeFrame, StructureError};
use leselang_syntax::{Expression, parse};

struct Token {
    bytes: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError(&'static str);
type Node = Computation<Token, Token, Token, Token>;
const LIMITS: GroupSourceLimits = GroupSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    max_branches: 64,
};
fn expression(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn source() -> Expression {
    expression("seq(first: device.open(), second: device.close())")
}
fn literal(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn token(drops: &Rc<Cell<usize>>) -> Token {
    Token {
        bytes: vec![7; 33],
        drops: drops.clone(),
    }
}
fn call(count: usize, drops: &Rc<Cell<usize>>) -> Node {
    Node::Call {
        operation: token(drops),
        arguments: (0..count)
            .map(|index| ComputedArgument {
                name: format!("value_{index}"),
                value: literal(index as u64),
            })
            .collect(),
    }
}
struct Adapter<'source> {
    input: &'source Expression,
    events: Vec<(GroupSourcePhase, usize)>,
    drops: Rc<Cell<usize>>,
    nodes: Vec<Option<Node>>,
    buffers: Vec<*const u8>,
    fail: Option<(GroupSourcePhase, usize)>,
    unwind: Option<(GroupSourcePhase, usize)>,
    scope: Vec<(&'source str, u8)>,
}
impl<'source> Adapter<'source> {
    fn new(input: &'source Expression) -> RefCell<Self> {
        RefCell::new(Self {
            input,
            events: Vec::new(),
            drops: Rc::new(Cell::new(0)),
            nodes: vec![None, None],
            buffers: Vec::new(),
            fail: None,
            unwind: None,
            scope: Vec::new(),
        })
    }
    fn step(&mut self, phase: GroupSourcePhase, index: usize) -> Result<(), PrivateError> {
        self.events.push((phase, index));
        let mut scope = ScopeFrame::new(&mut self.scope);
        let mut local = scope.nested();
        local.push("scratch", 3).unwrap();
        if self.unwind == Some((phase, index)) {
            panic!("private group unwind");
        }
        if self.fail == Some((phase, index)) {
            return Err(PrivateError("secret credential"));
        }
        Ok(())
    }
}
fn lower(
    adapter: &RefCell<Adapter<'_>>,
    limits: GroupSourceLimits,
) -> GroupSourceResult<Node, PrivateError> {
    let input = adapter.borrow().input;
    lower_flat_group_source(
        input,
        limits,
        |index, argument| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(GroupSourcePhase::Lower, index)?;
            let Expression::Call { arguments, .. } = input else {
                panic!()
            };
            assert!(std::ptr::eq(argument, &arguments[index]));
            let node = adapter
                .nodes
                .get_mut(index)
                .and_then(Option::take)
                .unwrap_or_else(|| Node::Host {
                    effect: Box::new(token(&adapter.drops)),
                });
            let result = token(&adapter.drops);
            adapter.buffers.push(result.bytes.as_ptr());
            Ok((node, result))
        },
        |index, argument, branch| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(GroupSourcePhase::Admit, index)?;
            let Expression::Call { arguments, .. } = input else {
                panic!()
            };
            assert!(std::ptr::eq(argument, &arguments[index]));
            assert_eq!(branch.name, argument.name);
            assert_eq!(branch.result_type.bytes.as_ptr(), adapter.buffers[index]);
            Ok(())
        },
    )
}
fn no_hooks(input: &Expression, limits: GroupSourceLimits) {
    let adapter = Adapter::new(input);
    assert!(lower(&adapter, limits).is_err());
    assert!(adapter.borrow().events.is_empty());
}

#[test]
fn source_group_labels_keep_stricter_lexical_grammar_than_ir_member_names() {
    for label in ["1", "all-4"] {
        let mut input = source();
        let Expression::Call { arguments, .. } = &mut input else {
            panic!()
        };
        arguments[1].name = label.into();
        let expected_span = arguments[1].span;
        let adapter = Adapter::new(&input);
        let failure = lower(&adapter, LIMITS).err().unwrap();
        assert!(matches!(failure, GroupSourceError::MemberName {
            kind: GroupKind::Sequence, index: 1, span,
        } if span == expected_span));
        assert!(adapter.borrow().events.is_empty());
        assert_eq!(adapter.borrow().drops.get(), 0);
    }
}

#[test]
fn original_named_arguments_move_only_results_and_two_stage_order_are_preserved() {
    for (text, kind) in [
        (
            "seq(second: device.close(), first: device.open())",
            GroupKind::Sequence,
        ),
        (
            "all(true: device.close(), false: device.open())",
            GroupKind::Parallel,
        ),
    ] {
        let input = expression(text);
        let adapter = Adapter::new(&input);
        let output = lower(&adapter, LIMITS).unwrap();
        assert_eq!(
            adapter.borrow().events,
            [
                (GroupSourcePhase::Lower, 0),
                (GroupSourcePhase::Lower, 1),
                (GroupSourcePhase::Admit, 0),
                (GroupSourcePhase::Admit, 1)
            ]
        );
        let Node::Group {
            group_kind,
            branches,
        } = &output
        else {
            panic!()
        };
        assert_eq!(*group_kind, kind);
        let Expression::Call { arguments, .. } = &input else {
            panic!()
        };
        for (index, branch) in branches.iter().enumerate() {
            assert_eq!(branch.name, arguments[index].name);
            assert_eq!(
                branch.result_type.bytes.as_ptr(),
                adapter.borrow().buffers[index]
            );
        }
        assert!(adapter.borrow().scope.is_empty());
        assert_eq!(adapter.borrow().drops.get(), 0);
        drop(output);
        assert_eq!(adapter.borrow().drops.get(), 4);
    }
}

#[test]
fn all_four_native_slots_nested_boxes_and_argument_buffers_move_without_clone() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    let field = token(&drops);
    let field_ptr = field.bytes.as_ptr();
    let operation = token(&drops);
    let operation_ptr = operation.bytes.as_ptr();
    let child = Box::new(literal(41));
    let child_ptr = child.as_ref() as *const Node;
    let arguments = vec![ComputedArgument {
        name: "value".into(),
        value: Node::Field {
            field,
            value: child,
        },
    }];
    let args_ptr = arguments.as_ptr();
    let effect = Box::new(token(&drops));
    let effect_ptr = effect.as_ref() as *const Token;
    adapter.borrow_mut().nodes = vec![
        Some(Node::Call {
            operation,
            arguments,
        }),
        Some(Node::Host { effect }),
    ];
    let output = lower(&adapter, LIMITS).unwrap();
    let Node::Group { branches, .. } = &output else {
        panic!()
    };
    let Node::Call {
        operation,
        arguments,
    } = &branches[0].value
    else {
        panic!()
    };
    assert_eq!(operation.bytes.as_ptr(), operation_ptr);
    assert_eq!(arguments.as_ptr(), args_ptr);
    let Node::Field { field, value } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), field_ptr);
    assert_eq!(value.as_ref() as *const Node, child_ptr);
    let Node::Host { effect } = &branches[1].value else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const Token, effect_ptr);
    drop(output);
    assert_eq!(drops.get(), 5);
}

#[test]
fn all_safety_ceiling_violations_precede_callbacks() {
    let input = source();
    for limits in [
        GroupSourceLimits {
            max_branches: 65,
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 16_385,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 16_385,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_arguments: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        let adapter = Adapter::new(&input);
        assert!(matches!(
            lower(&adapter, limits),
            Err(GroupSourceError::InvalidLimits)
        ));
        assert!(adapter.borrow().events.is_empty());
    }
}

#[test]
fn minimum_output_roots_depth_and_zero_branch_policies_are_explicit() {
    let input = source();
    for count in 0..3 {
        no_hooks(
            &input,
            GroupSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: count,
                    ..LIMITS.source
                },
                ..LIMITS
            },
        );
    }
    no_hooks(
        &input,
        GroupSourceLimits {
            max_branches: 0,
            ..LIMITS
        },
    );
    no_hooks(
        &input,
        GroupSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
    );
    assert!(
        lower(
            &Adapter::new(&input),
            GroupSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: 3,
                    max_lowered_depth: 1,
                    ..LIMITS.source
                },
                ..LIMITS
            }
        )
        .is_ok()
    );
}

#[test]
fn every_root_member_name_and_duplicate_is_checked_before_lowering() {
    for text in [
        "seq(a: device.open(), a: device.close())",
        "all(a: device.open(), a: device.close())",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
    for name in ["x".repeat(65), "bad-name".into()] {
        let mut input = source();
        let Expression::Call { arguments, .. } = &mut input else {
            panic!()
        };
        arguments[1].name = name;
        no_hooks(&input, LIMITS);
    }
}

#[test]
fn wrong_root_and_sequence_parallel_arity_are_never_defaulted() {
    for text in [
        "1",
        "device.open()",
        "repeat(times: 2, body: device.open())",
        "seq()",
        "all()",
        "all(a: device.open())",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
    for kind in ["seq", "all"] {
        let input = expression(&format!(
            "{kind}({})",
            (0..65)
                .map(|index| format!("v{index}: device.open()"))
                .collect::<Vec<_>>()
                .join(",")
        ));
        no_hooks(&input, LIMITS);
    }
    assert!(lower(&Adapter::new(&expression("seq(a: device.open())")), LIMITS).is_ok());
}

#[test]
fn whole_cold_source_text_names_nodes_depth_and_operand_counts_precede_hooks() {
    let input = expression("seq(a: device.open(), b: device.close(value: 1))");
    for limits in [
        GroupSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 3,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_arguments: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        no_hooks(&input, limits);
    }
    no_hooks(
        &expression(&format!(
            "seq(a: device.open(), b: device.close(value: \"{}\"))",
            "x".repeat(4097)
        )),
        LIMITS,
    );
    let mut bad = source();
    let Expression::Call { arguments, .. } = &mut bad else {
        panic!()
    };
    let Expression::Call { callee, .. } = &mut arguments[1].value else {
        panic!()
    };
    *callee = "bad-name".into();
    no_hooks(&bad, LIMITS);
}

#[test]
fn nested_cold_group_headers_are_validated_before_native_source_compilation() {
    for text in [
        "seq(a: device.open(value: all(one: 1)))",
        "seq(a: device.open(value: seq(one: 1, one: 2)))",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
}

#[test]
fn direct_nested_groups_and_repeat_require_an_explicit_outer_expansion_policy() {
    for text in [
        "seq(a: seq(b: device.open()))",
        "seq(a: repeat(times: 2, body: device.open()))",
        "all(a: device.open(), b: all(c: device.open(), d: device.close()))",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
}

#[test]
fn aggregate_first_output_reserves_future_member_roots_before_more_lowering() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[0] = Some(call(1, &drops));
    assert!(matches!(
        lower(
            &adapter,
            GroupSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: 3,
                    ..LIMITS.source
                },
                ..LIMITS
            }
        ),
        Err(GroupSourceError::Output { index: Some(0), .. })
    ));
    assert_eq!(adapter.borrow().events, [(GroupSourcePhase::Lower, 0)]);
    assert_eq!(adapter.borrow().drops.get(), 2);
}

#[test]
fn aggregate_later_output_is_bounded_before_any_admission_hook() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[1] = Some(call(1, &drops));
    assert!(matches!(
        lower(
            &adapter,
            GroupSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: 3,
                    ..LIMITS.source
                },
                ..LIMITS
            }
        ),
        Err(GroupSourceError::Output { index: Some(1), .. })
    ));
    assert_eq!(adapter.borrow().events.len(), 2);
    assert!(
        adapter
            .borrow()
            .events
            .iter()
            .all(|(phase, _)| *phase == GroupSourcePhase::Lower)
    );
    assert_eq!(adapter.borrow().drops.get(), 4);
}

#[test]
fn shifted_child_depth_stops_all_admission() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[0] = Some(call(1, &drops));
    assert!(matches!(
        lower(
            &adapter,
            GroupSourceLimits {
                source: SourceCallLimits {
                    max_lowered_depth: 1,
                    ..LIMITS.source
                },
                ..LIMITS
            }
        ),
        Err(GroupSourceError::Output {
            error: StructureError::DepthLimit,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events.len(), 1);
}

#[test]
fn pure_values_effectful_preparation_and_group_tails_are_not_atomic_candidates() {
    let input = source();
    for index in 0..3 {
        let adapter = Adapter::new(&input);
        let drops = adapter.borrow().drops.clone();
        adapter.borrow_mut().nodes[1] = Some(match index {
            0 => literal(1),
            1 => Node::Bind {
                name: "reply".into(),
                value: Box::new(Node::Host {
                    effect: Box::new(token(&drops)),
                }),
                body: Box::new(call(0, &drops)),
            },
            _ => Node::Group {
                group_kind: GroupKind::Sequence,
                branches: Vec::new(),
            },
        });
        assert!(matches!(
            lower(&adapter, LIMITS),
            Err(GroupSourceError::Candidate { index: 1, .. })
        ));
        assert_eq!(adapter.borrow().events.len(), 2);
    }
}

#[test]
fn zero_iteration_controls_cannot_hide_effects_in_call_preparation() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[1] = Some(Node::Call {
        operation: token(&drops),
        arguments: vec![ComputedArgument {
            name: "value".into(),
            value: Node::Loop {
                name: "state".into(),
                initial: Box::new(literal(0)),
                condition: Box::new(Node::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                next: Box::new(Node::Host {
                    effect: Box::new(token(&drops)),
                }),
                limit: 0,
            },
        }],
    });
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(GroupSourceError::Candidate { index: 1, .. })
    ));
    assert_eq!(adapter.borrow().events.len(), 2);
}

#[test]
fn pure_bind_choose_atomic_routes_retain_all_cold_children_without_selection() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[0] = Some(Node::Bind {
        name: "value".into(),
        value: Box::new(literal(7)),
        body: Box::new(Node::Choose {
            when: Box::new(Node::Literal {
                value: ScalarValue::Boolean(false),
            }),
            then: Box::new(call(0, &drops)),
            otherwise: Box::new(call(0, &drops)),
        }),
    });
    let output = lower(&adapter, LIMITS).unwrap();
    assert_eq!(adapter.borrow().events.len(), 4);
    drop(output);
    assert_eq!(drops.get(), 5);
}

#[test]
fn every_native_failure_keeps_phase_span_payload_but_redacts_display_and_source() {
    let input = source();
    for (phase, index, visits, drops) in [
        (GroupSourcePhase::Lower, 0, 1, 0),
        (GroupSourcePhase::Lower, 1, 2, 2),
        (GroupSourcePhase::Admit, 0, 3, 4),
        (GroupSourcePhase::Admit, 1, 4, 4),
    ] {
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().fail = Some((phase, index));
        let error = lower(&adapter, LIMITS).err().unwrap();
        assert!(!format!("{error:?} {error}").contains("secret credential"));
        assert!(std::error::Error::source(&error).is_none());
        let GroupSourceError::Native {
            phase: actual_phase,
            index: actual_index,
            span,
            error: PrivateError(payload),
        } = error
        else {
            panic!()
        };
        assert_eq!((actual_phase, actual_index), (phase, index));
        let Expression::Call { arguments, .. } = &input else {
            panic!()
        };
        assert_eq!(span, arguments[index].span);
        assert_eq!(payload, "secret credential");
        assert_eq!(adapter.borrow().events.len(), visits);
        assert_eq!(adapter.borrow().drops.get(), drops);
        assert!(adapter.borrow().scope.is_empty());
    }
}

#[test]
fn native_unwind_releases_all_consumed_parts_and_scopes_once_without_retry() {
    let input = source();
    for (phase, index, visits, drops) in [
        (GroupSourcePhase::Lower, 0, 1, 0),
        (GroupSourcePhase::Lower, 1, 2, 2),
        (GroupSourcePhase::Admit, 0, 3, 4),
        (GroupSourcePhase::Admit, 1, 4, 4),
    ] {
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().unwind = Some((phase, index));
        assert!(catch_unwind(AssertUnwindSafe(|| lower(&adapter, LIMITS))).is_err());
        assert_eq!(adapter.borrow().events.len(), visits);
        assert_eq!(adapter.borrow().drops.get(), drops);
        assert!(adapter.borrow().scope.is_empty());
    }
    assert!(lower(&Adapter::new(&input), LIMITS).is_ok());
}

#[test]
fn inclusive_sixty_four_member_limit_keeps_declaration_order_without_scheduler_work() {
    let input = expression(&format!(
        "all({})",
        (0..64)
            .map(|index| format!("v{index}: device.open()"))
            .collect::<Vec<_>>()
            .join(",")
    ));
    let adapter = Adapter::new(&input);
    let output = lower(&adapter, LIMITS).unwrap();
    assert_eq!(adapter.borrow().events.len(), 128);
    let Node::Group { branches, .. } = &output else {
        panic!()
    };
    for (index, branch) in branches.iter().enumerate() {
        assert_eq!(branch.name, format!("v{index}"));
    }
    drop(output);
    assert_eq!(adapter.borrow().drops.get(), 128);
}

#[test]
fn matching_result_observations_cannot_replace_uniform_atomic_operation_admission() {
    let input = source();
    let drops = Rc::new(Cell::new(0));
    let mut different = token(&drops);
    different.bytes[0] = 8;
    let mut first = Some(Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(call(0, &drops)),
        otherwise: Box::new(Node::Call {
            operation: different,
            arguments: Vec::new(),
        }),
    });
    let events = RefCell::new(Vec::new());
    let error = lower_flat_group_source(
        &input,
        LIMITS,
        |index, _| {
            events.borrow_mut().push((GroupSourcePhase::Lower, index));
            let node = if index == 0 {
                first.take().unwrap()
            } else {
                call(0, &drops)
            };
            Ok((node, token(&drops)))
        },
        |index, _, branch| {
            events.borrow_mut().push((GroupSourcePhase::Admit, index));
            let mut pending = vec![&branch.value];
            while let Some(node) = pending.pop() {
                if let Node::Call { operation, .. } = node {
                    if operation.bytes[0] != branch.result_type.bytes[0] {
                        return Err(PrivateError("different native operation identity"));
                    }
                } else {
                    pending.extend(node.children());
                }
            }
            Ok(())
        },
    )
    .err()
    .unwrap();
    assert!(matches!(
        error,
        GroupSourceError::Native {
            phase: GroupSourcePhase::Admit,
            index: 0,
            ..
        }
    ));
    assert_eq!(
        *events.borrow(),
        [
            (GroupSourcePhase::Lower, 0),
            (GroupSourcePhase::Lower, 1),
            (GroupSourcePhase::Admit, 0)
        ]
    );
    assert_eq!(drops.get(), 5);
}

#[test]
fn reference_flat_opaque_computed_and_expanded_groups_keep_wire_and_cold_authority() {
    for source in [
        "fn main() = bind(node: \"a\", body: seq(first: ui.focus(node_id: \"a\"), second: runtime.list()))",
        "fn main() = bind(node: \"a\", body: seq(first: ui.focus(node_id: node), second: ui.focus(node_id: \"b\")))",
        "fn main() = bind(node: \"a\", body: all(true: ui.focus(node_id: node), false: ui.focus(node_id: \"b\")))",
        "fn main() = bind(node: \"a\", body: seq(first: seq(one: ui.focus(node_id: node)), second: repeat(times: 2, body: ui.focus(node_id: node))))",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&leselang_hir::lower(&parse(&canonical)).unwrap()).unwrap()
        );
        leselang_hir::authorize(
            &program,
            &leselang_host_contract::CapabilitySet::new(["runtime.read", "ui.presentation"]),
        )
        .unwrap();
        assert!(
            leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
                .is_err()
        );
    }
}

#[test]
fn reference_nested_cold_error_indices_never_index_the_root_member_array() {
    for source in [
        "fn main() = bind(node: \"a\", body: seq(first: ui.focus(node_id: choose(when: true, then: \"a\", otherwise: seq(a: 1, b: 2, b: 3)))))",
        "fn main() = bind(node: \"a\", body: all(first: ui.focus(node_id: node), second: ui.focus(node_id: choose(when: true, then: \"a\", otherwise: seq(a: 1, b: 2, c: 3, c: 4)))))",
    ] {
        assert!(leselang_hir::lower(&parse(source)).is_err());
    }
}

mod counter {
    use super::*;
    use leselang_hir::call_evaluation::{
        CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope,
    };
    use leselang_hir::effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationLimits, EffectEvaluationOutcome,
        evaluate_effects_in_scope,
    };
    use leselang_hir::pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue,
    };
    use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
    use leselang_hir::source_call::{SourceCallHost, SourceSchema, lower_source_call};
    use leselang_runtime_core::*;

    struct Declaration {
        maximum: u64,
    }
    impl HostResultDomain<ScalarValue> for Declaration {
        type Error = PrivateError;
        fn matches_type(&self, value: &ScalarValue) -> bool {
            matches!(value, ScalarValue::Integer(_))
        }
        fn validate_value(&self, value: &ScalarValue) -> Result<(), PrivateError> {
            match value {
                ScalarValue::Integer(value) if *value <= self.maximum => Ok(()),
                _ => Err(PrivateError("private result domain")),
            }
        }
    }
    type Schema<'a> = SourceSchema<'a, &'static str, ScalarTypeSet, Declaration, u8>;
    type Ir<'a> = Computation<(), &'static str, (), &'a Schema<'a>>;
    const PURE: PureEvaluationLimits = PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    };
    const EFFECT: EffectEvaluationLimits = EffectEvaluationLimits {
        pure: PURE,
        max_arguments: 64,
        max_branches: 64,
    };
    fn compile<'schema>(
        input: &Expression,
        host: &SourceCallHost<'_, 'schema, &'static str, ScalarTypeSet, Declaration, u8>,
    ) -> GroupSourceResult<Ir<'schema>, PrivateError> {
        lower_flat_group_source(
            input,
            LIMITS,
            |_, argument| {
                let call = lower_source_call(&argument.value, host, LIMITS.source, |argument| {
                    lower_scalar_source_with_scope(
                        &argument.value,
                        ScalarSourceLimits {
                            source: LIMITS.source,
                            max_bindings: 8,
                        },
                        &[],
                        |_, _| -> Result<(Ir<'schema>, Option<ScalarType>), PrivateError> {
                            Err(PrivateError("unknown scalar alias"))
                        },
                    )
                    .map(|(value, ty)| (value, Some(ty)))
                })
                .map_err(|_| PrivateError("source schema rejected"))?;
                let schema = call.schema();
                Ok((
                    Ir::Call {
                        operation: schema.key,
                        arguments: call.into_arguments(),
                    },
                    schema,
                ))
            },
            |_, _, branch| {
                let Ir::Call { operation, .. } = &branch.value else {
                    return Err(PrivateError("not atomic"));
                };
                let original = host
                    .catalog
                    .authorize(operation, host.version, host.granted)
                    .map_err(|_| PrivateError("admission schema rejected"))?;
                if !std::ptr::eq(original, branch.result_type) {
                    return Err(PrivateError("wrong original declaration"));
                }
                Ok(())
            },
        )
    }
    struct Environment<'a> {
        catalog: &'a OperationCatalog<'a, &'static str, &'a str, ScalarTypeSet, Declaration, u8>,
        version: u32,
        grants: &'a [u8],
        events: RefCell<Vec<&'static str>>,
    }

    fn compile_sequence<'schema>(
        input: &Expression,
        host: &SourceCallHost<'_, 'schema, &'static str, ScalarTypeSet, Declaration, u8>,
    ) -> leselang_hir::sequence_source::SequenceSourceResult<Ir<'schema>, PrivateError> {
        use leselang_hir::sequence_source::{SequenceSourceMember, lower_sequence_source};
        lower_sequence_source(
            input,
            LIMITS,
            |_, argument| {
                if let Expression::Call { callee, .. } = &argument.value
                    && matches!(callee.as_str(), "seq" | "repeat")
                {
                    let nested = if callee == "seq" {
                        compile_sequence(&argument.value, host)
                            .map_err(|_| PrivateError("nested source rejected"))?
                    } else {
                        compile_repeated_sequence(&argument.value, host, false)
                            .map_err(|_| PrivateError("nested repeat rejected"))?
                    };
                    let Ir::Group { branches, .. } = nested else {
                        panic!()
                    };
                    return Ok(SequenceSourceMember::Sequence { branches });
                }
                let call = lower_source_call(&argument.value, host, LIMITS.source, |argument| {
                    lower_scalar_source_with_scope(
                        &argument.value,
                        ScalarSourceLimits {
                            source: LIMITS.source,
                            max_bindings: 8,
                        },
                        &[],
                        |_, _| -> Result<(Ir<'schema>, Option<ScalarType>), PrivateError> {
                            Err(PrivateError("unknown scalar alias"))
                        },
                    )
                    .map(|(value, ty)| (value, Some(ty)))
                })
                .map_err(|_| PrivateError("source schema rejected"))?;
                let schema = call.schema();
                Ok(SequenceSourceMember::Atomic {
                    value: Ir::Call {
                        operation: schema.key,
                        arguments: call.into_arguments(),
                    },
                    result_type: schema,
                })
            },
            |_, _, _, branch| {
                let Ir::Call { operation, .. } = &branch.value else {
                    return Err(PrivateError("not atomic"));
                };
                let schema = host
                    .catalog
                    .authorize(operation, host.version, host.granted)
                    .map_err(|_| PrivateError("admission schema rejected"))?;
                if !std::ptr::eq(schema, branch.result_type) {
                    return Err(PrivateError("wrong original declaration"));
                }
                Ok(())
            },
        )
    }

    struct NativeRepeat<'host, 'catalog, 'schema> {
        host:
            &'host SourceCallHost<'catalog, 'schema, &'static str, ScalarTypeSet, Declaration, u8>,
        corrupt: bool,
    }
    impl<'source, 'schema>
        leselang_hir::repeat_sequence_source::RepeatSequenceSourceAdapter<
            'source,
            (),
            &'static str,
            (),
            &'schema Schema<'schema>,
        > for NativeRepeat<'_, '_, 'schema>
    {
        type Error = PrivateError;
        fn lower_sequence(
            &mut self,
            body: &'source leselang_syntax::NamedArgument,
        ) -> Result<
            Vec<leselang_hir::ir::ComputedBranch<Ir<'schema>, &'schema Schema<'schema>>>,
            PrivateError,
        > {
            let Ir::Group { branches, .. } = compile_sequence(&body.value, self.host)
                .map_err(|_| PrivateError("native template rejected"))?
            else {
                panic!()
            };
            Ok(branches)
        }
        fn host_cost(
            &mut self,
            _: &(),
        ) -> Result<leselang_hir::source_cost::SourceCostExtra, PrivateError> {
            Err(PrivateError("opaque effects are not registered"))
        }
        fn materialize_sequence(
            &mut self,
            _: usize,
            _: &'source leselang_syntax::NamedArgument,
            original: &[leselang_hir::ir::ComputedBranch<Ir<'schema>, &'schema Schema<'schema>>],
        ) -> Result<
            Vec<leselang_hir::ir::ComputedBranch<Ir<'schema>, &'schema Schema<'schema>>>,
            PrivateError,
        > {
            let mut copy = original.to_vec();
            if self.corrupt {
                let Ir::Call { arguments, .. } = &mut copy[1].value else {
                    panic!()
                };
                arguments[0].value = Ir::Literal {
                    value: ScalarValue::Integer(12),
                };
            }
            Ok(copy)
        }
        fn admit_member(
            &mut self,
            _: usize,
            _: usize,
            _: &'source leselang_syntax::NamedArgument,
            original: &leselang_hir::ir::ComputedBranch<Ir<'schema>, &'schema Schema<'schema>>,
            candidate: &leselang_hir::ir::ComputedBranch<Ir<'schema>, &'schema Schema<'schema>>,
        ) -> Result<(), PrivateError> {
            let (
                Ir::Call {
                    operation,
                    arguments,
                },
                Ir::Call {
                    operation: expected,
                    arguments: original_args,
                },
            ) = (&candidate.value, &original.value)
            else {
                return Err(PrivateError("not registered atomic calls"));
            };
            let schema = self
                .host
                .catalog
                .authorize(operation, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("live native declaration rejected"))?;
            if operation != expected
                || !std::ptr::eq(schema, original.result_type)
                || !std::ptr::eq(schema, candidate.result_type)
                || arguments.len() != original_args.len()
            {
                return Err(PrivateError("wrong native declaration"));
            }
            let environment = Environment {
                catalog: self.host.catalog,
                version: self.host.version,
                grants: self.host.granted,
                events: RefCell::new(Vec::new()),
            };
            let scalar = |value: &Ir<'schema>| {
                let mut values = Vec::new();
                let mut scope = ScopeFrame::new(&mut values);
                let mut fuel = Fuel::new(256);
                match leselang_hir::pure_evaluation::evaluate_pure_in_scope(
                    value,
                    &mut scope,
                    &environment,
                    &mut fuel,
                    PURE,
                ) {
                    Ok(PureValue::Scalar(value)) => Ok(value),
                    _ => Err(PrivateError("native scalar corroboration failed")),
                }
            };
            for (argument, expected) in arguments.iter().zip(original_args) {
                if argument.name != expected.name
                    || scalar(&argument.value)? != scalar(&expected.value)?
                {
                    return Err(PrivateError("changed native argument semantics"));
                }
            }
            Ok(())
        }
    }
    fn compile_repeated_sequence<'schema>(
        input: &Expression,
        host: &SourceCallHost<'_, 'schema, &'static str, ScalarTypeSet, Declaration, u8>,
        corrupt: bool,
    ) -> leselang_hir::repeat_sequence_source::RepeatSequenceResult<Ir<'schema>, PrivateError> {
        leselang_hir::repeat_sequence_source::lower_repeat_sequence_source(
            input,
            leselang_hir::repeat_sequence_source::RepeatSequenceSourceLimits {
                repeat: leselang_hir::repeat_source::RepeatSourceLimits {
                    source: LIMITS.source,
                    expanded: leselang_hir::source_cost::SourceCostLimits {
                        max_nodes: 256,
                        max_depth: 32,
                    },
                    max_repetitions: 64,
                },
                max_branches: 64,
            },
            &mut NativeRepeat { host, corrupt },
        )
    }
    impl PureEvaluationEnvironment<(), &'static str> for Environment<'_> {
        type Result = ();
        type Error = PrivateError;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, PrivateError> {
            Err(PrivateError("no fields"))
        }
        fn member(&self, _: &(), _: &str, _: &&'static str) -> Result<(), PrivateError> {
            Err(PrivateError("no received group"))
        }
    }
    impl leselang_hir::pure_typing::PureTypeEnvironment<(), &'static str> for Environment<'_> {
        type Result = ();
        fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &(), _: &str, _: &&'static str) -> Option<()> {
            None
        }
    }
    struct SequentialEnvironment<'host, 'schema> {
        inner: &'host Environment<'schema>,
        owner: u64,
    }
    impl<'expression, 'schema: 'expression>
        leselang_hir::sequence_evaluation::SequenceEvaluationEnvironment<
            'expression,
            (),
            &'static str,
            (),
            &'schema Schema<'schema>,
        > for SequentialEnvironment<'_, 'schema>
    {
        type Identity = (u64, usize);
        type Declaration = Declaration;
        type Request = PreparedCall<'schema, &'static str, ScalarTypeSet, Declaration, u8>;
        type Error = PrivateError;
        fn preflight_member(
            &self,
            _: usize,
            branch: &'expression leselang_hir::ir::ComputedBranch<
                Ir<'schema>,
                &'schema Schema<'schema>,
            >,
        ) -> Result<(), PrivateError> {
            use leselang_hir::call_typing::{CallTypeHost, CallTypeLimits, infer_call_type};
            self.inner.events.borrow_mut().push("sequence preflight");
            let declaration = infer_call_type(
                &branch.value,
                &[],
                &CallTypeHost {
                    catalog: self.inner.catalog,
                    version: self.inner.version,
                    granted: self.inner.grants,
                    environment: self.inner,
                },
                CallTypeLimits {
                    pure: leselang_hir::pure_typing::TypeInferenceLimits {
                        max_nodes: PURE.max_nodes,
                        max_depth: PURE.max_depth,
                        max_bindings: PURE.max_bindings,
                    },
                    max_arguments: 64,
                },
            )
            .map_err(|_| PrivateError("cold sequence schema or type rejected"))?;
            if !std::ptr::eq(declaration, &branch.result_type.result) {
                return Err(PrivateError("wrong sequence declaration"));
            }
            Ok(())
        }
        fn prepare_member(
            &self,
            index: usize,
            branch: &'expression leselang_hir::ir::ComputedBranch<
                Ir<'schema>,
                &'schema Schema<'schema>,
            >,
            fuel: &mut Fuel,
        ) -> Result<
            leselang_hir::sequence_evaluation::PreparedSequenceMember<
                'expression,
                (u64, usize),
                Declaration,
                Self::Request,
            >,
            PrivateError,
        > {
            self.inner.events.borrow_mut().push("sequence prepare");
            let Ir::Call {
                operation,
                arguments,
            } = &branch.value
            else {
                return Err(PrivateError("not a registered atomic call"));
            };
            let schema = self
                .inner
                .catalog
                .authorize(operation, self.inner.version, self.inner.grants)
                .map_err(|_| PrivateError("live sequence schema rejected"))?;
            if !std::ptr::eq(schema, branch.result_type) {
                return Err(PrivateError("live original sequence declaration changed"));
            }
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let request = evaluate_call_arguments_in_scope(
                arguments,
                schema,
                &mut scope,
                self.inner,
                fuel,
                CallEvaluationLimits {
                    pure: PURE,
                    max_arguments: 64,
                },
            )
            .map_err(|_| PrivateError("actual sequence argument rejected"))?;
            Ok(leselang_hir::sequence_evaluation::PreparedSequenceMember {
                identity: (self.owner, index),
                declaration: &schema.result,
                request,
            })
        }
    }
    struct SequenceAuthority<'ir, 'host, 'schema> {
        environment: &'host Environment<'schema>,
        owner: u64,
        branches: &'ir [leselang_hir::ir::ComputedBranch<Ir<'schema>, &'schema Schema<'schema>>],
    }
    #[derive(Default)]
    struct NativeCounters {
        left: u64,
        right: u64,
        calls: Vec<&'static str>,
    }
    impl NativeCounters {
        fn invoke(
            &mut self,
            request: PreparedCall<'_, &'static str, ScalarTypeSet, Declaration, u8>,
        ) -> Result<ScalarValue, PrivateError> {
            let (schema, arguments) = request.into_parts();
            let ScalarValue::Integer(value) = arguments
                .into_iter()
                .next()
                .ok_or(PrivateError("missing native counter input"))?
                .value
            else {
                return Err(PrivateError("wrong native counter input"));
            };
            let counter = match schema.key {
                "counter.left" => &mut self.left,
                "counter.right" => &mut self.right,
                _ => return Err(PrivateError("unknown native counter operation")),
            };
            *counter = counter
                .checked_add(value)
                .ok_or(PrivateError("native counter overflow"))?;
            self.calls.push(schema.key);
            Ok(ScalarValue::Integer(*counter))
        }
    }
    impl ReplyAuthority<(u64, usize)> for SequenceAuthority<'_, '_, '_> {
        type Error = PrivateError;
        fn authorize(&self, &(owner, index): &(u64, usize)) -> Result<(), PrivateError> {
            if owner != self.owner {
                return Err(PrivateError("wrong execution generation"));
            }
            let branch = self
                .branches
                .get(index)
                .ok_or(PrivateError("wrong sequence index"))?;
            let Ir::Call { operation, .. } = &branch.value else {
                return Err(PrivateError("wrong original call"));
            };
            let schema = self
                .environment
                .catalog
                .authorize(operation, self.environment.version, self.environment.grants)
                .map_err(|_| PrivateError("reply version or grant revoked"))?;
            if !std::ptr::eq(schema, branch.result_type) {
                return Err(PrivateError("reply original declaration changed"));
            }
            Ok(())
        }
    }
    struct Request<'a> {
        kind: GroupKind,
        calls: Vec<PreparedCall<'a, &'static str, ScalarTypeSet, Declaration, u8>>,
    }
    impl<'expression, 'schema>
        EffectEvaluationEnvironment<'expression, (), &'static str, (), &'schema Schema<'schema>>
        for Environment<'schema>
    {
        type Request = Request<'schema>;
        type Capture = ();
        fn preflight_effect(
            &self,
            expression: &'expression Ir<'schema>,
        ) -> Result<(), PrivateError> {
            if let Ir::Call {
                operation,
                arguments,
            } = expression
            {
                self.events.borrow_mut().push("call");
                let schema = self
                    .catalog
                    .authorize(operation, self.version, self.grants)
                    .map_err(|_| PrivateError("live call schema rejected"))?;
                schema
                    .bind_arguments(
                        &arguments
                            .iter()
                            .map(|argument| argument.name.as_str())
                            .collect::<Vec<_>>(),
                    )
                    .map_err(|_| PrivateError("wrong call names"))?;
                return Ok(());
            }
            self.events.borrow_mut().push("group");
            let Ir::Group { branches, .. } = expression else {
                return Err(PrivateError("not group"));
            };
            for branch in branches {
                let Ir::Call {
                    operation,
                    arguments,
                } = &branch.value
                else {
                    return Err(PrivateError("not atomic"));
                };
                let schema = self
                    .catalog
                    .authorize(operation, self.version, self.grants)
                    .map_err(|_| PrivateError("live schema rejected"))?;
                if !std::ptr::eq(schema, branch.result_type) {
                    return Err(PrivateError("forged declaration"));
                }
                schema
                    .bind_arguments(
                        &arguments
                            .iter()
                            .map(|argument| argument.name.as_str())
                            .collect::<Vec<_>>(),
                    )
                    .map_err(|_| PrivateError("wrong names"))?;
            }
            Ok(())
        }
        fn prepare_effect(
            &self,
            expression: &'expression Ir<'schema>,
            scope: &mut ScopeFrame<'_, 'expression, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Request<'schema>, CalculationFailure<PrivateError>> {
            self.events.borrow_mut().push("prepare");
            let Ir::Group {
                group_kind,
                branches,
            } = expression
            else {
                return Err(PrivateError("not group").into());
            };
            let mut calls = Vec::new();
            for branch in branches {
                let Ir::Call {
                    operation,
                    arguments,
                } = &branch.value
                else {
                    return Err(PrivateError("not atomic").into());
                };
                let schema = self
                    .catalog
                    .authorize(operation, self.version, self.grants)
                    .map_err(|_| PrivateError("live schema rejected"))?;
                fuel.charge(1)
                    .map_err(|_| PrivateError("native group call fuel exhausted"))?;
                calls.push(
                    evaluate_call_arguments_in_scope(
                        arguments,
                        schema,
                        scope,
                        self,
                        fuel,
                        CallEvaluationLimits {
                            pure: PURE,
                            max_arguments: 64,
                        },
                    )
                    .map_err(|_| PrivateError("evaluated arguments rejected"))?,
                );
            }
            Ok(Request {
                kind: *group_kind,
                calls,
            })
        }
        fn capture(
            &self,
            _: &'expression str,
            _: &'expression Ir<'schema>,
            _: &ScopeFrame<'_, 'expression, PureValue<()>>,
            _: &mut Fuel,
        ) -> Result<(), PrivateError> {
            Err(PrivateError("group proof has no suspension"))
        }
    }

    #[test]
    fn parsed_native_groups_preserve_original_rows_prepare_real_values_and_exact_fuel_without_dispatch()
     {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration { maximum: 100 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let host = SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        };
        for (name, kind) in [("seq", GroupKind::Sequence), ("all", GroupKind::Parallel)] {
            let input = expression(&format!(
                "{name}(first: counter.left(value: add(left: 40, right: 2)), second: counter.right(value: 11))"
            ));
            let output = compile(&input, &host).unwrap();
            let Ir::Group { branches, .. } = &output else {
                panic!()
            };
            assert!(std::ptr::eq(branches[0].result_type, &schemas[0]));
            assert!(std::ptr::eq(branches[1].result_type, &schemas[1]));
            let environment = Environment {
                catalog: &catalog,
                version: 7,
                grants: &[31, 32],
                events: RefCell::new(Vec::new()),
            };
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let mut fuel = Fuel::new(100);
            let outcome =
                evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                    .unwrap();
            let EffectEvaluationOutcome::Request(request) = outcome else {
                panic!()
            };
            assert_eq!(request.kind, kind);
            assert_eq!(
                *environment.events.borrow(),
                ["group", "call", "call", "prepare"]
            );
            let mut direct_fuel = Fuel::new(100);
            for (index, prepared) in request.calls.iter().enumerate() {
                assert!(std::ptr::eq(prepared.schema(), &schemas[index]));
                assert_eq!(
                    prepared.arguments()[0].value,
                    ScalarValue::Integer(if index == 0 { 42 } else { 11 })
                );
                prepared
                    .schema()
                    .check_result(&ScalarValue::Integer(42))
                    .unwrap();
                assert!(
                    prepared
                        .schema()
                        .check_result(&ScalarValue::Boolean(true))
                        .is_err()
                );
                assert!(
                    prepared
                        .schema()
                        .check_result(&ScalarValue::Integer(101))
                        .is_err()
                );
                let Ir::Call { arguments, .. } = &branches[index].value else {
                    panic!()
                };
                let direct = evaluate_call_arguments_in_scope(
                    arguments,
                    &schemas[index],
                    &mut scope,
                    &environment,
                    &mut direct_fuel,
                    CallEvaluationLimits {
                        pure: PURE,
                        max_arguments: 64,
                    },
                )
                .unwrap();
                assert_eq!(direct.arguments()[0].value, prepared.arguments()[0].value);
            }
            assert_eq!(
                direct_fuel.remaining() - fuel.remaining(),
                3,
                "one Group root and two native Call roots"
            );
            assert!(scope.is_empty());
        }
    }

    #[test]
    fn missing_later_grants_stale_versions_and_wrong_rows_fail_before_value_preparation() {
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &[],
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &[],
                required_capability: 32,
                result: Declaration { maximum: 100 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 0,
            },
        )
        .unwrap();
        let input = expression("seq(first: counter.left(), second: counter.right())");
        for (version, granted, index) in [(8, &[31, 32][..], 0), (7, &[31][..], 1)] {
            assert!(
                matches!(compile(&input, &SourceCallHost { catalog: &catalog, version, granted }),
                Err(GroupSourceError::Native { phase: GroupSourcePhase::Lower, index: actual, .. }) if actual == index)
            );
        }
        let host = SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        };
        let mut output = compile(&input, &host).unwrap();
        for (version, grants) in [(8, &[31, 32][..]), (7, &[31][..])] {
            let environment = Environment {
                catalog: &catalog,
                version,
                grants,
                events: RefCell::new(Vec::new()),
            };
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let mut fuel = Fuel::new(100);
            assert!(
                evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                    .is_err()
            );
            assert_eq!(*environment.events.borrow(), ["group"]);
            assert_eq!(fuel.remaining(), 100);
        }
        let Ir::Group { branches, .. } = &mut output else {
            panic!()
        };
        branches[1].result_type = &schemas[0];
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let mut fuel = Fuel::new(100);
        assert!(
            evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                .is_err()
        );
        assert_eq!(*environment.events.borrow(), ["group"]);
        assert_eq!(fuel.remaining(), 100);
    }

    #[test]
    fn parsed_nested_sequence_preserves_two_native_schemas_values_fuel_and_live_rejection() {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration { maximum: 200 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let host = SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        };
        let input = expression(
            "seq(inner: seq(deeper: seq(left: counter.left(value: add(left: 40, right: 2)))), right: counter.right(value: 11))",
        );
        let mut output = compile_sequence(&input, &host).unwrap();
        let Ir::Group { branches, .. } = &output else {
            panic!()
        };
        assert_eq!(branches[0].name, "inner__deeper__left");
        assert_eq!(branches[1].name, "right");
        assert!(std::ptr::eq(branches[0].result_type, &schemas[0]));
        assert!(std::ptr::eq(branches[1].result_type, &schemas[1]));
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let mut fuel = Fuel::new(100);
        let EffectEvaluationOutcome::Request(request) =
            evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                .unwrap()
        else {
            panic!()
        };
        assert_eq!(request.kind, GroupKind::Sequence);
        assert_eq!(
            *environment.events.borrow(),
            ["group", "call", "call", "prepare"]
        );
        assert_eq!(fuel.remaining(), 93);
        for (index, call) in request.calls.iter().enumerate() {
            assert!(std::ptr::eq(call.schema(), &schemas[index]));
            assert_eq!(
                call.arguments()[0].value,
                ScalarValue::Integer(if index == 0 { 42 } else { 11 })
            );
            assert!(
                call.schema()
                    .check_result(&ScalarValue::Boolean(true))
                    .is_err()
            );
        }
        assert!(
            request.calls[0]
                .schema()
                .check_result(&ScalarValue::Integer(150))
                .is_err()
        );
        request.calls[1]
            .schema()
            .check_result(&ScalarValue::Integer(150))
            .unwrap();
        for (version, grants) in [(8, &[31, 32][..]), (7, &[31][..])] {
            let environment = Environment {
                catalog: &catalog,
                version,
                grants,
                events: RefCell::new(Vec::new()),
            };
            let mut fuel = Fuel::new(100);
            assert!(
                evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                    .is_err()
            );
            assert_eq!(*environment.events.borrow(), ["group"]);
            assert_eq!(fuel.remaining(), 100);
        }
        drop(request);
        drop(scope);
        drop(values);
        let Ir::Group { branches, .. } = &mut output else {
            panic!()
        };
        branches[1].result_type = &schemas[0];
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let mut fuel = Fuel::new(100);
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        assert!(
            evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                .is_err()
        );
        assert_eq!(*environment.events.borrow(), ["group"]);
        assert_eq!(fuel.remaining(), 100);
        assert!(scope.is_empty());
    }

    #[test]
    fn parsed_repeated_sequence_preserves_native_rows_values_exact_fuel_and_rejects_changed_semantics()
     {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration { maximum: 200 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let host = SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        };
        let input = expression(
            "repeat(times: 2, body: seq(inner: seq(left: counter.left(value: add(left: 40, right: 2))), right: counter.right(value: 11)))",
        );
        assert!(matches!(
            compile_repeated_sequence(&input, &host, true),
            Err(
                leselang_hir::repeat_sequence_source::RepeatSequenceSourceError::Admission {
                    iteration: 2,
                    member: 1,
                    error: PrivateError("changed native argument semantics"),
                    ..
                }
            )
        ));
        let output = compile_repeated_sequence(&input, &host, false).unwrap();
        let Ir::Group { branches, .. } = &output else {
            panic!()
        };
        assert_eq!(
            branches
                .iter()
                .map(|branch| branch.name.as_str())
                .collect::<Vec<_>>(),
            [
                "iteration_1__inner__left",
                "iteration_1__right",
                "iteration_2__inner__left",
                "iteration_2__right"
            ]
        );
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let mut fuel = Fuel::new(100);
        let EffectEvaluationOutcome::Request(request) =
            evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                .unwrap()
        else {
            panic!()
        };
        assert_eq!(fuel.remaining(), 87);
        assert_eq!(request.calls.len(), 4);
        for (index, call) in request.calls.iter().enumerate() {
            assert!(std::ptr::eq(
                branches[index].result_type,
                &schemas[index % 2]
            ));
            assert!(std::ptr::eq(call.schema(), &schemas[index % 2]));
            assert_eq!(
                call.arguments()[0].value,
                ScalarValue::Integer(if index % 2 == 0 { 42 } else { 11 })
            );
            assert!(
                call.schema()
                    .check_result(&ScalarValue::Boolean(true))
                    .is_err()
            );
        }
        assert!(
            request.calls[0]
                .schema()
                .check_result(&ScalarValue::Integer(150))
                .is_err()
        );
        request.calls[1]
            .schema()
            .check_result(&ScalarValue::Integer(150))
            .unwrap();
        for (version, grants) in [(8, &[31, 32][..]), (7, &[31][..])] {
            let environment = Environment {
                catalog: &catalog,
                version,
                grants,
                events: RefCell::new(Vec::new()),
            };
            let mut fuel = Fuel::new(100);
            assert!(
                evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                    .is_err()
            );
            assert_eq!(fuel.remaining(), 100);
            assert_eq!(*environment.events.borrow(), ["group"]);
        }
        assert!(scope.is_empty());
    }

    #[test]
    fn parsed_nested_repeat_runs_one_native_request_per_accepted_reply_with_exact_fuel() {
        use leselang_hir::sequence_evaluation::*;
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration { maximum: 200 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let host = SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        };
        let input = expression(
            "seq(start: counter.left(value: 1), nested: repeat(times: 2, body: seq(inner: seq(left: counter.left(value: add(left: 40, right: 2))), right: counter.right(value: 11))))",
        );
        let output = compile_sequence(&input, &host).unwrap();
        let Ir::Group { branches, .. } = &output else {
            panic!()
        };
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let native = SequentialEnvironment {
            inner: &environment,
            owner: 53,
        };
        let authority = SequenceAuthority {
            environment: &environment,
            owner: 53,
            branches,
        };
        let mut session = SequenceEvaluation::start(
            &output,
            &native,
            Fuel::new(100),
            SequenceEvaluationLimits {
                max_nodes: 256,
                max_depth: 32,
                max_branches: 64,
            },
        )
        .unwrap();
        assert_eq!(*environment.events.borrow(), ["sequence preflight"; 5]);
        assert_eq!(session.fuel_remaining(), 99);
        let mut counters = NativeCounters::default();
        for (index, expected) in [1, 42, 11, 42, 11].into_iter().enumerate() {
            let SequencePoll::Request {
                index: position,
                request,
            } = session.poll(&native).unwrap()
            else {
                panic!()
            };
            assert_eq!(position, index);
            assert!(std::ptr::eq(request.schema(), branches[index].result_type));
            assert_eq!(request.arguments()[0].value, ScalarValue::Integer(expected));
            let fuel = session.fuel_remaining();
            let events = environment.events.borrow().clone();
            for _ in 0..2 {
                assert!(
                    matches!(session.poll(&native).unwrap(), SequencePoll::Awaiting { index: waiting } if waiting == index)
                );
            }
            assert_eq!(*environment.events.borrow(), events);
            assert_eq!(session.fuel_remaining(), fuel);
            assert_eq!(counters.calls.len(), index);

            // Invocation is outside the language; the prepared request is consumed once.
            let schema = request.schema();
            let result = counters
                .invoke(request)
                .unwrap_or_else(|_| panic!("native invocation rejected"));
            let expected_reply = [1, 43, 11, 85, 22][index];
            assert_eq!(result, ScalarValue::Integer(expected_reply));
            assert!(matches!(
                session
                    .try_accept::<_, ScalarValue, _>(
                        &(54, index),
                        ScalarValue::Integer(expected_reply),
                        &authority
                    )
                    .unwrap_err()
                    .error,
                SequenceReplyError::Reply(ReplyAcceptanceError::IdentityMismatch)
            ));
            assert!(
                session
                    .try_accept::<_, ScalarValue, _>(
                        &(53, index),
                        ScalarValue::Boolean(true),
                        &authority
                    )
                    .is_err()
            );
            let accepted = session
                .try_accept::<_, ScalarValue, _>(&(53, index), result, &authority)
                .unwrap();
            let (position, original, identity, domain, reply) = accepted.into_parts();
            assert_eq!(position, index);
            assert!(std::ptr::eq(original, &branches[index]));
            assert!(std::ptr::eq(domain, &schema.result));
            assert_eq!(identity, (53, index));
            assert_eq!(reply, ScalarValue::Integer(expected_reply));
            assert_eq!(session.fuel_remaining(), fuel);
            assert!(
                session
                    .try_accept::<_, ScalarValue, _>(
                        &(53, index),
                        ScalarValue::Integer(expected),
                        &authority
                    )
                    .is_err()
            );
            assert_eq!(counters.calls.len(), index + 1);
        }
        assert_eq!(session.fuel_remaining(), 85);
        assert_eq!(
            session.status(),
            SequenceStatus::Terminal(SequenceEnd::Completed)
        );
        assert!(matches!(
            session.poll(&native).unwrap(),
            SequencePoll::Terminal(SequenceEnd::Completed)
        ));
        assert!(!session.cancel());
        assert_eq!(
            environment
                .events
                .borrow()
                .iter()
                .filter(|&&event| event == "sequence prepare")
                .count(),
            5
        );
        assert_eq!(
            counters.calls,
            [
                "counter.left",
                "counter.left",
                "counter.right",
                "counter.left",
                "counter.right"
            ]
        );
        assert_eq!((counters.left, counters.right), (85, 22));
    }

    #[test]
    fn parsed_sequence_revocation_cancellation_and_stale_replies_never_invoke_a_successor() {
        use leselang_hir::sequence_evaluation::*;
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration { maximum: 200 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let host = SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        };
        let output = compile_sequence(
            &expression("seq(left: counter.left(value: 42), right: counter.right(value: 11))"),
            &host,
        )
        .unwrap();
        let Ir::Group { branches, .. } = &output else {
            panic!()
        };
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let native = SequentialEnvironment {
            inner: &environment,
            owner: 81,
        };
        let authority = SequenceAuthority {
            environment: &environment,
            owner: 81,
            branches,
        };
        let limits = SequenceEvaluationLimits {
            max_nodes: 256,
            max_depth: 32,
            max_branches: 64,
        };
        let mut session =
            SequenceEvaluation::start(&output, &native, Fuel::new(100), limits).unwrap();
        let SequencePoll::Request { request, .. } = session.poll(&native).unwrap() else {
            panic!()
        };
        let mut counters = NativeCounters::default();
        assert_eq!(
            counters
                .invoke(request)
                .unwrap_or_else(|_| panic!("native invocation rejected")),
            ScalarValue::Integer(42)
        );
        let revoked = Environment {
            catalog: &catalog,
            version: 8,
            grants: &[31, 32],
            events: RefCell::new(Vec::new()),
        };
        let policy = SequenceAuthority {
            environment: &revoked,
            owner: 81,
            branches,
        };
        assert!(matches!(
            session
                .try_accept::<_, ScalarValue, _>(&(81, 0), ScalarValue::Integer(42), &policy)
                .unwrap_err()
                .error,
            SequenceReplyError::Reply(ReplyAcceptanceError::Authority(_))
        ));
        assert_eq!(session.status(), SequenceStatus::Awaiting { index: 0 });
        assert_eq!(session.fuel_remaining(), 97);
        drop(
            session
                .try_accept::<_, ScalarValue, _>(&(81, 0), ScalarValue::Integer(42), &authority)
                .unwrap(),
        );
        let revoked = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31],
            events: RefCell::new(Vec::new()),
        };
        assert!(
            session
                .poll(&SequentialEnvironment {
                    inner: &revoked,
                    owner: 81
                })
                .is_err()
        );
        assert_eq!(
            session.status(),
            SequenceStatus::Terminal(SequenceEnd::Failed)
        );
        assert_eq!(session.fuel_remaining(), 96);
        assert!(matches!(
            session.poll(&native).unwrap(),
            SequencePoll::Terminal(SequenceEnd::Failed)
        ));
        assert_eq!(counters.calls, ["counter.left"]);
        assert_eq!((counters.left, counters.right), (42, 0));

        let mut cancelled =
            SequenceEvaluation::start(&output, &native, Fuel::new(100), limits).unwrap();
        let SequencePoll::Request { request, .. } = cancelled.poll(&native).unwrap() else {
            panic!()
        };
        let late_reply = counters
            .invoke(request)
            .unwrap_or_else(|_| panic!("native invocation rejected"));
        assert!(cancelled.cancel());
        assert!(
            cancelled
                .try_accept::<_, ScalarValue, _>(&(81, 0), late_reply, &authority)
                .is_err()
        );
        assert!(matches!(
            cancelled.poll(&native).unwrap(),
            SequencePoll::Terminal(SequenceEnd::Cancelled)
        ));
        assert_eq!(counters.calls, ["counter.left", "counter.left"]);
        assert_eq!((counters.left, counters.right), (84, 0));
        assert!(!cancelled.cancel());

        for (version, grants) in [(8, &[31, 32][..]), (7, &[31][..])] {
            let invalid = Environment {
                catalog: &catalog,
                version,
                grants,
                events: RefCell::new(Vec::new()),
            };
            assert!(
                SequenceEvaluation::start(
                    &output,
                    &SequentialEnvironment {
                        inner: &invalid,
                        owner: 81
                    },
                    Fuel::new(100),
                    limits
                )
                .is_err()
            );
            assert!(!invalid.events.borrow().contains(&"sequence prepare"));
        }
    }
}
