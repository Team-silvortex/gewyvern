use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::group_source::{GroupSourceLimits, GroupSourcePhase};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::sequence_source::{
    SequenceSourceError, SequenceSourceMember, SequenceSourceResult, lower_sequence_source,
};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{ScalarValue, StructureError};
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
type Member = SequenceSourceMember<Node, Token>;
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
    expression("seq(inner: seq(one: device.read(), two: device.read()), tail: device.read())")
}
fn token(drops: &Rc<Cell<usize>>) -> Token {
    Token {
        bytes: vec![7; 33],
        drops: drops.clone(),
    }
}
fn branch(name: &str, drops: &Rc<Cell<usize>>) -> ComputedBranch<Node, Token> {
    ComputedBranch {
        name: name.into(),
        value: Node::Host {
            effect: Box::new(token(drops)),
        },
        result_type: token(drops),
    }
}
struct Adapter<'a> {
    source: &'a Expression,
    drops: Rc<Cell<usize>>,
    events: Vec<(GroupSourcePhase, usize, Option<usize>)>,
    overrides: Vec<Option<Member>>,
    buffers: Vec<*const u8>,
    fail: Option<(GroupSourcePhase, usize, Option<usize>)>,
    unwind: bool,
}
impl<'a> Adapter<'a> {
    fn new(source: &'a Expression) -> RefCell<Self> {
        RefCell::new(Self {
            source,
            drops: Rc::new(Cell::new(0)),
            events: Vec::new(),
            overrides: Vec::new(),
            buffers: Vec::new(),
            fail: None,
            unwind: false,
        })
    }
    fn step(
        &mut self,
        phase: GroupSourcePhase,
        index: usize,
        member: Option<usize>,
    ) -> Result<(), PrivateError> {
        self.events.push((phase, index, member));
        if self.fail == Some((phase, index, member)) {
            if self.unwind {
                panic!("private sequence unwind");
            }
            return Err(PrivateError("secret credential"));
        }
        Ok(())
    }
}
fn lower(
    adapter: &RefCell<Adapter<'_>>,
    limits: GroupSourceLimits,
) -> SequenceSourceResult<Node, PrivateError> {
    let input = adapter.borrow().source;
    lower_sequence_source(
        input,
        limits,
        |index, argument| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(GroupSourcePhase::Lower, index, None)?;
            let Expression::Call { arguments, .. } = input else {
                panic!()
            };
            assert!(std::ptr::eq(argument, &arguments[index]));
            let value = if let Some(value) = adapter.overrides.get_mut(index).and_then(Option::take)
            {
                value
            } else if let Expression::Call {
                callee, arguments, ..
            } = &argument.value
                && matches!(callee.as_str(), "seq" | "repeat")
            {
                Member::Sequence {
                    branches: arguments
                        .iter()
                        .map(|argument| branch(&argument.name, &adapter.drops))
                        .collect(),
                }
            } else {
                let result_type = token(&adapter.drops);
                Member::Atomic {
                    value: Node::Host {
                        effect: Box::new(token(&adapter.drops)),
                    },
                    result_type,
                }
            };
            match &value {
                Member::Atomic { result_type, .. } => {
                    adapter.buffers.push(result_type.bytes.as_ptr())
                }
                Member::Sequence { branches } => adapter.buffers.extend(
                    branches
                        .iter()
                        .map(|branch| branch.result_type.bytes.as_ptr()),
                ),
            }
            Ok(value)
        },
        |index, member, argument, branch| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(GroupSourcePhase::Admit, index, Some(member))?;
            let Expression::Call { arguments, .. } = input else {
                panic!()
            };
            assert!(std::ptr::eq(argument, &arguments[index]));
            assert!(adapter.buffers.contains(&branch.result_type.bytes.as_ptr()));
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
fn original_rows_order_labels_and_move_only_results_survive_flattening() {
    let input = source();
    let adapter = Adapter::new(&input);
    let output = lower(&adapter, LIMITS).unwrap();
    assert_eq!(
        adapter.borrow().events,
        [
            (GroupSourcePhase::Lower, 0, None),
            (GroupSourcePhase::Lower, 1, None),
            (GroupSourcePhase::Admit, 0, Some(0)),
            (GroupSourcePhase::Admit, 0, Some(1)),
            (GroupSourcePhase::Admit, 1, Some(0)),
        ]
    );
    let Node::Group {
        group_kind,
        branches,
    } = &output
    else {
        panic!()
    };
    assert_eq!(*group_kind, GroupKind::Sequence);
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>(),
        ["inner__one", "inner__two", "tail"]
    );
    for (branch, pointer) in branches.iter().zip(&adapter.borrow().buffers) {
        assert_eq!(branch.result_type.bytes.as_ptr(), *pointer);
    }
    assert_eq!(adapter.borrow().drops.get(), 0);
    drop(output);
    assert_eq!(adapter.borrow().drops.get(), 6);
}

#[test]
fn all_four_native_slots_and_original_boxes_and_operand_buffers_are_moved() {
    let input = expression("seq(inner: seq(one: device.read()))");
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    let field = token(&drops);
    let field_pointer = field.bytes.as_ptr();
    let operation = token(&drops);
    let operation_pointer = operation.bytes.as_ptr();
    let child = Box::new(Node::Literal {
        value: ScalarValue::Integer(42),
    });
    let child_pointer = child.as_ref() as *const Node;
    let arguments = vec![ComputedArgument {
        name: "value".into(),
        value: Node::Field {
            field,
            value: child,
        },
    }];
    let arguments_pointer = arguments.as_ptr();
    let effect = Box::new(token(&drops));
    let effect_pointer = effect.as_ref() as *const Token;
    adapter.borrow_mut().overrides.push(Some(Member::Sequence {
        branches: vec![
            ComputedBranch {
                name: "one".into(),
                value: Node::Call {
                    operation,
                    arguments,
                },
                result_type: token(&drops),
            },
            ComputedBranch {
                name: "two".into(),
                value: Node::Host { effect },
                result_type: token(&drops),
            },
        ],
    }));
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
    assert_eq!(operation.bytes.as_ptr(), operation_pointer);
    assert_eq!(arguments.as_ptr(), arguments_pointer);
    let Node::Field { field, value } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), field_pointer);
    assert_eq!(value.as_ref() as *const Node, child_pointer);
    let Node::Host { effect } = &branches[1].value else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const Token, effect_pointer);
    drop(output);
    assert_eq!(drops.get(), 5);
}

#[test]
fn complete_cold_ast_headers_and_names_fail_before_any_lowering() {
    for text in [
        "seq()",
        "seq(a: device.read(), a: seq(b: device.read()))",
        "all(a: device.read(), b: device.read())",
        "seq(a: device.read(value: seq()))",
        "seq(a: device.read(value: all(x: 1)))",
        "seq(a: device.read(value: seq(x: 1, x: 2)))",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
    for text in [
        format!("seq(a: device.read(value: \"{}\"))", "x".repeat(4097)),
        format!("seq({}: seq(a: device.read()))", "x".repeat(65)),
    ] {
        no_hooks(&expression(&text), LIMITS);
    }
}

#[test]
fn zero_and_ceiling_limits_fail_without_callbacks() {
    let input = source();
    for limits in [
        GroupSourceLimits {
            max_branches: 0,
            ..LIMITS
        },
        GroupSourceLimits {
            max_branches: 65,
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 0,
                ..LIMITS.source
            },
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
                max_source_depth: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
        GroupSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 0,
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
                max_lowered_depth: 0,
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
        no_hooks(&input, limits);
    }
}

#[test]
fn parallel_members_are_never_silently_serialized_or_lowered() {
    let input = expression("seq(a: device.read(), b: all(x: device.read(), y: device.read()))");
    let adapter = Adapter::new(&input);
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(SequenceSourceError::ParallelMember { index: 1, .. })
    ));
    assert!(adapter.borrow().events.is_empty());
}

#[test]
fn native_member_kind_must_match_the_original_source_role() {
    for (input, member) in [
        (source(), false),
        (expression("seq(a: device.read())"), true),
    ] {
        let adapter = Adapter::new(&input);
        let drops = adapter.borrow().drops.clone();
        adapter.borrow_mut().overrides.push(Some(if member {
            Member::Sequence {
                branches: vec![branch("one", &drops)],
            }
        } else {
            Member::Atomic {
                value: Node::Host {
                    effect: Box::new(token(&drops)),
                },
                result_type: token(&drops),
            }
        }));
        assert!(matches!(
            lower(&adapter, LIMITS),
            Err(SequenceSourceError::MemberKind { index: 0, .. })
        ));
        assert_eq!(adapter.borrow().events.len(), 1);
        assert_eq!(drops.get(), 2);
    }
}

#[test]
fn empty_or_overwide_children_stop_before_later_lowering_and_admission() {
    for count in [0, 64, 65] {
        let input = source();
        let adapter = Adapter::new(&input);
        let drops = adapter.borrow().drops.clone();
        adapter.borrow_mut().overrides.push(Some(Member::Sequence {
            branches: (0..count)
                .map(|index| branch(&format!("b{index}"), &drops))
                .collect(),
        }));
        assert!(matches!(
            lower(&adapter, LIMITS),
            Err(SequenceSourceError::Width { index: 0, .. })
        ));
        assert_eq!(
            adapter.borrow().events,
            [(GroupSourcePhase::Lower, 0, None)]
        );
        assert_eq!(drops.get(), count * 2);
    }
}

#[test]
fn inclusive_expanded_width_accepts_sixty_four_and_refuses_sixty_five() {
    for (width, success) in [(64, true), (65, false)] {
        let input = expression("seq(inner: seq(one: device.read()))");
        let adapter = Adapter::new(&input);
        let drops = adapter.borrow().drops.clone();
        adapter.borrow_mut().overrides.push(Some(Member::Sequence {
            branches: (0..width)
                .map(|index| branch(&format!("b{index}"), &drops))
                .collect(),
        }));
        let output = lower(&adapter, LIMITS);
        assert_eq!(output.is_ok(), success);
        assert_eq!(adapter.borrow().events.len(), if success { 65 } else { 1 });
        drop(output);
        assert_eq!(drops.get(), width * 2);
    }
}

#[test]
fn joined_names_are_bounded_before_allocation_and_collisions_before_admission() {
    for (parent, child, success) in [
        ("p".repeat(31), "c".repeat(31), true),
        ("p".repeat(32), "c".repeat(31), false),
        ("p".into(), "bad-name".into(), false),
    ] {
        let input = expression(&format!("seq({parent}: seq(one: device.read()))"));
        let adapter = Adapter::new(&input);
        let drops = adapter.borrow().drops.clone();
        adapter.borrow_mut().overrides.push(Some(Member::Sequence {
            branches: vec![branch(&child, &drops)],
        }));
        let output = lower(&adapter, LIMITS);
        assert_eq!(output.is_ok(), success);
        if !success {
            assert!(matches!(
                output,
                Err(SequenceSourceError::Name {
                    index: 0,
                    member: 0,
                    ..
                })
            ));
        }
    }
    for text in [
        "seq(inner__one: device.read(), inner: seq(one: device.read()))",
        "seq(inner: seq(one: device.read()), inner__one: device.read())",
    ] {
        let input = expression(text);
        let adapter = Adapter::new(&input);
        assert!(matches!(
            lower(&adapter, LIMITS),
            Err(SequenceSourceError::Collision {
                index: 1,
                member: 0,
                ..
            })
        ));
        assert_eq!(adapter.borrow().events.len(), 2);
        assert_eq!(adapter.borrow().drops.get(), 4);
    }
}

#[test]
fn child_internal_duplicate_labels_are_not_hidden_by_prefixing() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().overrides.push(Some(Member::Sequence {
        branches: vec![branch("one", &drops), branch("one", &drops)],
    }));
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(SequenceSourceError::Collision {
            index: 0,
            member: 1,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events.len(), 1);
    assert_eq!(drops.get(), 4);
}

#[test]
fn whole_shifted_output_reserves_future_roots_before_later_lowering() {
    for (nodes, success) in [(4, true), (3, false)] {
        let input = source();
        let adapter = Adapter::new(&input);
        let limits = GroupSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: nodes,
                ..LIMITS.source
            },
            ..LIMITS
        };
        let output = lower(&adapter, limits);
        assert_eq!(output.is_ok(), success);
        if !success {
            assert!(matches!(
                output,
                Err(SequenceSourceError::Output {
                    index: Some(0),
                    error: StructureError::NodeLimit
                })
            ));
            assert_eq!(adapter.borrow().events.len(), 1);
            assert_eq!(adapter.borrow().drops.get(), 4);
        }
    }
}

#[test]
fn shifted_depth_and_all_candidates_precede_every_admission() {
    for (value, depth, expected_depth) in [
        (
            Node::Group {
                group_kind: GroupKind::Sequence,
                branches: vec![],
            },
            32,
            false,
        ),
        (
            Node::Literal {
                value: ScalarValue::Integer(1),
            },
            32,
            false,
        ),
        (
            Node::Call {
                operation: token(&Rc::new(Cell::new(0))),
                arguments: vec![ComputedArgument {
                    name: "value".into(),
                    value: Node::Literal {
                        value: ScalarValue::Integer(1),
                    },
                }],
            },
            1,
            true,
        ),
    ] {
        let input = source();
        let adapter = Adapter::new(&input);
        let drops = adapter.borrow().drops.clone();
        adapter.borrow_mut().overrides = vec![
            None,
            Some(Member::Atomic {
                value,
                result_type: token(&drops),
            }),
        ];
        let output = lower(
            &adapter,
            GroupSourceLimits {
                source: SourceCallLimits {
                    max_lowered_depth: depth,
                    ..LIMITS.source
                },
                ..LIMITS
            },
        );
        if expected_depth {
            assert!(matches!(
                output,
                Err(SequenceSourceError::Output {
                    index: Some(1),
                    error: StructureError::DepthLimit
                })
            ));
        } else {
            assert!(matches!(
                output,
                Err(SequenceSourceError::Candidate { index: 1, .. })
            ));
        }
        assert_eq!(adapter.borrow().events.len(), 2);
    }
}

#[test]
fn native_failures_are_redacted_indexed_and_stop_later_hooks() {
    for phase in [
        (GroupSourcePhase::Lower, 1, None),
        (GroupSourcePhase::Admit, 0, Some(0)),
        (GroupSourcePhase::Admit, 0, Some(1)),
        (GroupSourcePhase::Admit, 1, Some(0)),
    ] {
        let input = source();
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().fail = Some(phase);
        let error = lower(&adapter, LIMITS).err().unwrap();
        assert!(!format!("{error:?} {error}").contains("secret"));
        assert!(std::error::Error::source(&error).is_none());
        let SequenceSourceError::Native {
            phase: actual,
            index,
            member,
            error: PrivateError(secret),
            ..
        } = error
        else {
            panic!()
        };
        assert_eq!((actual, index, member), phase);
        assert_eq!(secret, "secret credential");
        assert_eq!(adapter.borrow().events.last(), Some(&phase));
        assert_eq!(
            adapter.borrow().drops.get(),
            if phase.0 == GroupSourcePhase::Lower {
                4
            } else {
                6
            }
        );
    }
}

#[test]
fn native_unwind_drops_all_consumed_rows_once_and_never_retries() {
    for phase in [
        (GroupSourcePhase::Lower, 1, None),
        (GroupSourcePhase::Admit, 0, Some(1)),
    ] {
        let input = source();
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().fail = Some(phase);
        adapter.borrow_mut().unwind = true;
        assert!(catch_unwind(AssertUnwindSafe(|| lower(&adapter, LIMITS))).is_err());
        assert_eq!(adapter.borrow().events.last(), Some(&phase));
        assert_eq!(
            adapter.borrow().drops.get(),
            if phase.0 == GroupSourcePhase::Lower {
                4
            } else {
                6
            }
        );
    }
}

#[test]
fn reference_nested_sequences_keep_canonical_wire_authority_and_diagnostics() {
    for body in [
        "seq(inner: seq(one: ui.focus(node_id: node)), tail: runtime.list())",
        "seq(inner: repeat(times: 2, body: ui.focus(node_id: node)), tail: ui.focus(node_id: \"b\"))",
        "seq(inner: seq(deep: seq(one: ui.focus(node_id: node))), tail: runtime.list())",
        "seq(inner: repeat(times: 2, body: seq(one: ui.focus(node_id: node), two: runtime.list())))",
    ] {
        let program = leselang_hir::lower(&parse(&format!(
            "fn main() = bind(node: \"a\", body: {body})"
        )))
        .unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let reparsed = leselang_hir::lower(&parse(&canonical)).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&reparsed).unwrap()
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
    for (body, code, message) in [
        (
            "seq(a: seq(x: ui.focus(node_id: node)), b: all(x: runtime.list(), y: runtime.list()))",
            "LSH1301",
            "all cannot be nested inside sequential control flow",
        ),
        (
            "seq(a: seq(x: ui.focus(node_id: node)), a__x: runtime.list())",
            "LSH1301",
            "expanded step names must be unique and at most 64 bytes",
        ),
    ] {
        let diagnostics = leselang_hir::lower(&parse(&format!(
            "fn main() = bind(node: \"a\", body: {body})"
        )))
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.message == message)
        );
    }
}
