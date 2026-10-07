use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::repeat_sequence_source::{
    RepeatSequenceResult, RepeatSequenceSourceAdapter, RepeatSequenceSourceError,
    RepeatSequenceSourceLimits, lower_repeat_sequence_source,
};
use leselang_hir::repeat_source::{RepeatSourceError, RepeatSourceLimits, RepeatSourcePhase};
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::{SourceCostError, SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::{ScalarValue, ScopeFrame, StructureError};
use leselang_syntax::{Expression, NamedArgument, parse};

struct Token {
    id: usize,
    bytes: Vec<u8>,
    extra: SourceCostExtra,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError(&'static str);
type Node = Computation<Token, Token, Token, Token>;
type Branch = ComputedBranch<Node, Token>;
const LIMITS: RepeatSequenceSourceLimits = RepeatSequenceSourceLimits {
    repeat: RepeatSourceLimits {
        source: SourceCallLimits {
            max_source_nodes: 256,
            max_source_depth: 32,
            max_lowered_nodes: 256,
            max_lowered_depth: 32,
            max_arguments: 64,
        },
        expanded: SourceCostLimits {
            max_nodes: 256,
            max_depth: 32,
        },
        max_repetitions: 64,
    },
    max_branches: 64,
};
fn expression(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn source(count: usize) -> Expression {
    expression(&format!(
        "repeat(times: {count}, body: seq(one: device.read(), two: device.write()))"
    ))
}
fn token(id: usize, drops: &Rc<Cell<usize>>) -> Token {
    Token {
        id,
        bytes: vec![7; 33],
        extra: SourceCostExtra { nodes: 0, depth: 0 },
        drops: drops.clone(),
    }
}
fn branch(name: &str, id: usize, drops: &Rc<Cell<usize>>) -> Branch {
    Branch {
        name: name.into(),
        value: Node::Host {
            effect: Box::new(token(id, drops)),
        },
        result_type: token(id, drops),
    }
}
#[derive(Clone, Copy)]
enum Change {
    Width,
    Name,
    Shape,
    Cost,
    Candidate,
    Operation,
    Result,
}
struct Adapter<'a> {
    source: &'a Expression,
    drops: Rc<Cell<usize>>,
    events: Vec<(RepeatSourcePhase, usize, usize)>,
    cost_calls: usize,
    cost_order: Vec<usize>,
    original_buffers: Vec<*const u8>,
    lowered: Option<Vec<Branch>>,
    change: Option<Change>,
    fail: Option<(RepeatSourcePhase, usize, usize)>,
    cost_fail: Option<usize>,
    unwind: bool,
    scope: Vec<(&'a str, u8)>,
}
impl<'a> Adapter<'a> {
    fn new(source: &'a Expression) -> Self {
        Self {
            source,
            drops: Rc::new(Cell::new(0)),
            events: Vec::new(),
            cost_calls: 0,
            cost_order: Vec::new(),
            original_buffers: Vec::new(),
            lowered: None,
            change: None,
            fail: None,
            cost_fail: None,
            unwind: false,
            scope: Vec::new(),
        }
    }
    fn step(
        &mut self,
        phase: RepeatSourcePhase,
        iteration: usize,
        member: usize,
    ) -> Result<(), PrivateError> {
        self.events.push((phase, iteration, member));
        let mut scope = ScopeFrame::new(&mut self.scope);
        scope.push("scratch", 1).unwrap();
        if self.fail == Some((phase, iteration, member)) {
            if self.unwind {
                panic!("private native unwind");
            }
            return Err(PrivateError("secret credential"));
        }
        Ok(())
    }
    fn body(&self) -> &NamedArgument {
        let Expression::Call { arguments, .. } = self.source else {
            panic!()
        };
        arguments
            .iter()
            .find(|argument| argument.name == "body")
            .unwrap()
    }
}
fn native_operation(node: &Node) -> Option<&Token> {
    match node {
        Node::Host { effect } => Some(effect),
        Node::Call { operation, .. } => Some(operation),
        _ => None,
    }
}
impl<'a> RepeatSequenceSourceAdapter<'a, Token, Token, Token, Token> for Adapter<'a> {
    type Error = PrivateError;
    fn lower_sequence(&mut self, body: &'a NamedArgument) -> Result<Vec<Branch>, PrivateError> {
        self.step(RepeatSourcePhase::Lower, 1, 0)?;
        assert!(std::ptr::eq(body, self.body()));
        let branches = self
            .lowered
            .take()
            .unwrap_or_else(|| vec![branch("one", 1, &self.drops), branch("two", 2, &self.drops)]);
        self.original_buffers = branches
            .iter()
            .map(|branch| branch.result_type.bytes.as_ptr())
            .collect();
        Ok(branches)
    }
    fn host_cost(&mut self, effect: &Token) -> Result<SourceCostExtra, PrivateError> {
        self.cost_calls += 1;
        self.cost_order.push(effect.id);
        let mut scope = ScopeFrame::new(&mut self.scope);
        scope.push("cost", 2).unwrap();
        if self.cost_fail == Some(self.cost_calls) {
            if self.unwind {
                panic!("private native cost unwind");
            }
            return Err(PrivateError("private cost credential"));
        }
        Ok(effect.extra)
    }
    fn materialize_sequence(
        &mut self,
        iteration: usize,
        body: &'a NamedArgument,
        original: &[Branch],
    ) -> Result<Vec<Branch>, PrivateError> {
        self.step(RepeatSourcePhase::Materialize, iteration, 0)?;
        assert!(std::ptr::eq(body, self.body()));
        for (branch, pointer) in original.iter().zip(&self.original_buffers) {
            assert_eq!(branch.result_type.bytes.as_ptr(), *pointer);
            assert!(!branch.name.starts_with("iteration_"));
        }
        let mut branches = original
            .iter()
            .map(|original| {
                let mut copy = branch(
                    &original.name,
                    native_operation(&original.value).unwrap().id,
                    &self.drops,
                );
                copy.result_type.id = original.result_type.id;
                if let Node::Host { effect: original } = &original.value {
                    let Node::Host { effect } = &mut copy.value else {
                        panic!()
                    };
                    effect.extra = original.extra;
                }
                copy
            })
            .collect::<Vec<_>>();
        if iteration == 2
            && let Some(change) = self.change
        {
            match change {
                Change::Width => {
                    branches.pop();
                }
                Change::Name => branches[0].name = "renamed".into(),
                Change::Shape => {
                    branches[0].value = Node::Call {
                        operation: token(1, &self.drops),
                        arguments: vec![ComputedArgument {
                            name: "value".into(),
                            value: Node::Literal {
                                value: ScalarValue::Integer(1),
                            },
                        }],
                    }
                }
                Change::Cost => {
                    let Node::Host { effect } = &mut branches[0].value else {
                        panic!()
                    };
                    effect.extra.nodes += 1;
                }
                Change::Candidate => {
                    branches[0].value = Node::Literal {
                        value: ScalarValue::Integer(1),
                    }
                }
                Change::Operation => {
                    let Node::Host { effect } = &mut branches[0].value else {
                        panic!()
                    };
                    effect.id = 9;
                }
                Change::Result => branches[0].result_type.id = 9,
            }
        }
        Ok(branches)
    }
    fn admit_member(
        &mut self,
        iteration: usize,
        member: usize,
        body: &'a NamedArgument,
        original: &Branch,
        candidate: &Branch,
    ) -> Result<(), PrivateError> {
        self.step(RepeatSourcePhase::Admit, iteration, member)?;
        assert!(std::ptr::eq(body, self.body()));
        assert_eq!(
            original.result_type.bytes.as_ptr(),
            self.original_buffers[member]
        );
        if iteration == 1 {
            assert!(std::ptr::eq(original, candidate));
        }
        if native_operation(&original.value).map(|operation| operation.id)
            != native_operation(&candidate.value).map(|operation| operation.id)
            || original.result_type.id != candidate.result_type.id
        {
            return Err(PrivateError("wrong native identity"));
        }
        Ok(())
    }
}
fn lower(
    adapter: &mut Adapter<'_>,
    limits: RepeatSequenceSourceLimits,
) -> RepeatSequenceResult<Node, PrivateError> {
    lower_repeat_sequence_source(adapter.source, limits, adapter)
}
fn no_hooks(input: &Expression, limits: RepeatSequenceSourceLimits) {
    let mut adapter = Adapter::new(input);
    assert!(lower(&mut adapter, limits).is_err());
    assert!(adapter.events.is_empty());
    assert_eq!(adapter.cost_calls, 0);
}

#[test]
fn original_template_rows_move_once_and_factories_precede_all_ordered_admission() {
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    let output = lower(&mut adapter, LIMITS).unwrap();
    assert_eq!(
        &adapter.events[..3],
        [
            (RepeatSourcePhase::Lower, 1, 0),
            (RepeatSourcePhase::Materialize, 2, 0),
            (RepeatSourcePhase::Materialize, 3, 0)
        ]
    );
    assert_eq!(
        &adapter.events[3..],
        (1..=3)
            .flat_map(|iteration| (0..2).map(move |member| (
                RepeatSourcePhase::Admit,
                iteration,
                member
            )))
            .collect::<Vec<_>>()
    );
    assert_eq!(adapter.cost_order, [2, 1, 2, 1, 2, 1]);
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
        [
            "iteration_1__one",
            "iteration_1__two",
            "iteration_2__one",
            "iteration_2__two",
            "iteration_3__one",
            "iteration_3__two"
        ]
    );
    assert_eq!(
        branches[0].result_type.bytes.as_ptr(),
        adapter.original_buffers[0]
    );
    assert_eq!(
        branches[1].result_type.bytes.as_ptr(),
        adapter.original_buffers[1]
    );
    assert!(adapter.scope.is_empty());
    assert_eq!(adapter.drops.get(), 0);
    drop(output);
    assert_eq!(adapter.drops.get(), 12);
}

#[test]
fn one_iteration_needs_no_factory_and_sixty_four_retains_the_first_template() {
    for count in [1, 64] {
        let input = source(count);
        let mut adapter = Adapter::new(&input);
        adapter.lowered = Some(vec![branch("one", 1, &adapter.drops)]);
        let output = lower(&mut adapter, LIMITS).unwrap();
        assert_eq!(
            adapter
                .events
                .iter()
                .filter(|event| event.0 == RepeatSourcePhase::Materialize)
                .count(),
            count - 1
        );
        let Node::Group { branches, .. } = &output else {
            panic!()
        };
        assert_eq!(branches.len(), count);
        assert_eq!(
            branches[0].result_type.bytes.as_ptr(),
            adapter.original_buffers[0]
        );
        assert_eq!(branches[count - 1].name, format!("iteration_{count}__one"));
        drop(output);
        assert_eq!(adapter.drops.get(), count * 2);
    }
}

#[test]
fn all_four_native_slots_boxes_and_operand_buffers_require_no_clone_debug_serde_or_send() {
    let input = source(1);
    let mut adapter = Adapter::new(&input);
    let field = token(7, &adapter.drops);
    let field_pointer = field.bytes.as_ptr();
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
    let args_pointer = arguments.as_ptr();
    let operation = token(1, &adapter.drops);
    let operation_pointer = operation.bytes.as_ptr();
    let effect = Box::new(token(2, &adapter.drops));
    let effect_pointer = effect.as_ref() as *const Token;
    adapter.lowered = Some(vec![
        Branch {
            name: "one".into(),
            value: Node::Call {
                operation,
                arguments,
            },
            result_type: token(1, &adapter.drops),
        },
        Branch {
            name: "two".into(),
            value: Node::Host { effect },
            result_type: token(2, &adapter.drops),
        },
    ]);
    let output = lower(&mut adapter, LIMITS).unwrap();
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
    assert_eq!(arguments.as_ptr(), args_pointer);
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
    assert_eq!(adapter.drops.get(), 5);
}

#[test]
fn every_cold_repeat_and_group_header_and_whole_ast_precedes_lowering() {
    for text in [
        "seq(one: device.read())",
        "repeat(times: 0, body: seq(one: device.read()))",
        "repeat(times: 65, body: seq(one: device.read()))",
        "repeat(times: add(left: 1, right: 1), body: seq(one: device.read()))",
        "repeat(times: 2, body: seq())",
        "repeat(times: 2, body: seq(one: 1, one: 2))",
        "repeat(times: 2, body: seq(one: device.read(value: repeat(times: 0, body: device.read()))))",
        "repeat(times: 2, body: seq(one: device.read(value: all(one: 1))))",
        "repeat(times: 2, body: device.read())",
        "repeat(times: 2, body: all(one: device.read(), two: device.write()))",
        "repeat(times: 2, body: seq(one: device.read()), extra: 1)",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
    no_hooks(
        &expression(&format!(
            "repeat(times: 2, body: seq(one: device.read(value: \"{}\")))",
            "x".repeat(4097)
        )),
        LIMITS,
    );
}

#[test]
fn all_safety_ceilings_and_zero_minimum_reservations_reject_without_callbacks() {
    let input = source(3);
    for limits in [
        RepeatSequenceSourceLimits {
            max_branches: 65,
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            max_branches: 0,
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                max_repetitions: 65,
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                max_repetitions: 0,
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 16_385,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_source_depth: 65,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_arguments: 65,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: 16_385,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_lowered_depth: 65,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                expanded: SourceCostLimits {
                    max_nodes: 16_385,
                    ..LIMITS.repeat.expanded
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                expanded: SourceCostLimits {
                    max_depth: 65,
                    ..LIMITS.repeat.expanded
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: 3,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_lowered_depth: 0,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                expanded: SourceCostLimits {
                    max_nodes: 3,
                    ..LIMITS.repeat.expanded
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
        RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                expanded: SourceCostLimits {
                    max_depth: 0,
                    ..LIMITS.repeat.expanded
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        },
    ] {
        no_hooks(&input, limits);
    }
}

#[test]
fn empty_templates_and_total_expanded_width_stop_before_factories() {
    for width in [0, 22, 65] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.lowered = Some(
            (0..width)
                .map(|index| branch(&format!("b{index}"), index, &adapter.drops))
                .collect(),
        );
        assert!(matches!(
            lower(&mut adapter, LIMITS),
            Err(RepeatSequenceSourceError::Width { iteration: 1, .. })
        ));
        assert_eq!(adapter.events.len(), 1);
        assert_eq!(adapter.cost_calls, 0);
        assert_eq!(adapter.drops.get(), width * 2);
    }
}

#[test]
fn two_member_templates_accept_exactly_sixty_four_expanded_branches() {
    for (count, success) in [(32, true), (33, false)] {
        let input = source(count);
        let mut adapter = Adapter::new(&input);
        let output = lower(&mut adapter, LIMITS);
        assert_eq!(output.is_ok(), success);
        if success {
            let Node::Group { branches, .. } = output.unwrap() else {
                panic!()
            };
            assert_eq!(branches.len(), 64);
            assert_eq!(branches[63].name, "iteration_32__two");
            drop(branches);
            assert_eq!(adapter.drops.get(), 128);
        } else {
            assert_eq!(adapter.events.len(), 1);
            assert_eq!(adapter.cost_calls, 0);
        }
    }
}

#[test]
fn shifted_native_source_depth_is_not_repeated_group_nesting_or_physical_depth() {
    for (depth, success) in [(3, true), (2, false)] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        let mut branches = vec![
            branch("one", 1, &adapter.drops),
            branch("two", 2, &adapter.drops),
        ];
        let Node::Host { effect } = &mut branches[0].value else {
            panic!()
        };
        effect.extra.depth = 2;
        adapter.lowered = Some(branches);
        let limits = RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_lowered_depth: 1,
                    ..LIMITS.repeat.source
                },
                expanded: SourceCostLimits {
                    max_depth: depth,
                    ..LIMITS.repeat.expanded
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        };
        let output = lower(&mut adapter, limits);
        assert_eq!(output.is_ok(), success);
        if !success {
            assert!(matches!(
                output,
                Err(RepeatSequenceSourceError::Repeat(RepeatSourceError::Cost {
                    iteration: 1,
                    error: SourceCostError::Structure(StructureError::DepthLimit),
                }))
            ));
            assert_eq!(adapter.events.len(), 1);
            assert_eq!(adapter.cost_calls, 2);
        }
    }
}

#[test]
fn flattened_physical_reservation_does_not_recharge_removed_group_roots() {
    for (nodes, success) in [(7, true), (6, false)] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        let limits = RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: nodes,
                    ..LIMITS.repeat.source
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        };
        let output = lower(&mut adapter, limits);
        assert_eq!(output.is_ok(), success);
        if !success {
            assert!(matches!(
                output,
                Err(RepeatSequenceSourceError::Repeat(
                    RepeatSourceError::Output {
                        iteration: 1,
                        error: StructureError::NodeLimit
                    }
                ))
            ));
            assert_eq!(adapter.events.len(), 1);
            assert_eq!(adapter.cost_calls, 0);
        }
    }
}

#[test]
fn opaque_source_reservation_is_complete_before_any_instance_factory() {
    for (nodes, success) in [(19, true), (18, false)] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        let mut branches = vec![
            branch("one", 1, &adapter.drops),
            branch("two", 2, &adapter.drops),
        ];
        for branch in &mut branches {
            let Node::Host { effect } = &mut branch.value else {
                panic!()
            };
            effect.extra.nodes = 2;
        }
        adapter.lowered = Some(branches);
        let limits = RepeatSequenceSourceLimits {
            repeat: RepeatSourceLimits {
                expanded: SourceCostLimits {
                    max_nodes: nodes,
                    ..LIMITS.repeat.expanded
                },
                ..LIMITS.repeat
            },
            ..LIMITS
        };
        let output = lower(&mut adapter, limits);
        assert_eq!(output.is_ok(), success);
        if !success {
            assert_eq!(adapter.events.len(), 1);
            assert_eq!(adapter.cost_calls, 2);
        }
    }
}

#[test]
fn folded_source_costs_cannot_be_replaced_by_small_physical_shapes() {
    let input = source(2);
    let mut adapter = Adapter::new(&input);
    adapter.lowered = Some(vec![
        Branch {
            name: "one".into(),
            value: Node::Call {
                operation: token(1, &adapter.drops),
                arguments: vec![ComputedArgument {
                    name: "value".into(),
                    value: Node::Literal {
                        value: ScalarValue::OptionalString(
                            leselang_runtime_core::OptionalStringValue(None),
                        ),
                    },
                }],
            },
            result_type: token(1, &adapter.drops),
        },
        branch("two", 2, &adapter.drops),
    ]);
    let limits = RepeatSequenceSourceLimits {
        repeat: RepeatSourceLimits {
            expanded: SourceCostLimits {
                max_nodes: 8,
                ..LIMITS.repeat.expanded
            },
            ..LIMITS.repeat
        },
        ..LIMITS
    };
    assert!(matches!(
        lower(&mut adapter, limits),
        Err(RepeatSequenceSourceError::Repeat(
            RepeatSourceError::Output {
                iteration: 1,
                error: StructureError::NodeLimit
            }
        ))
    ));
    assert_eq!(adapter.events.len(), 1);
}

#[test]
fn longest_iteration_label_and_template_collisions_are_checked_before_factories() {
    for (size, success) in [(50, true), (51, false)] {
        let input = source(10);
        let mut adapter = Adapter::new(&input);
        adapter.lowered = Some(vec![branch(&"x".repeat(size), 1, &adapter.drops)]);
        let output = lower(&mut adapter, LIMITS);
        assert_eq!(output.is_ok(), success);
        if !success {
            assert!(matches!(
                output,
                Err(RepeatSequenceSourceError::Name {
                    iteration: 1,
                    member: 0,
                    ..
                })
            ));
            assert_eq!(adapter.cost_calls, 0);
            assert_eq!(adapter.events.len(), 1);
        }
    }
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    adapter.lowered = Some(vec![
        branch("one", 1, &adapter.drops),
        branch("one", 2, &adapter.drops),
    ]);
    assert!(matches!(
        lower(&mut adapter, LIMITS),
        Err(RepeatSequenceSourceError::Name { member: 1, .. })
    ));
    assert_eq!(adapter.cost_calls, 0);
}

#[test]
fn changed_width_labels_shape_cost_or_candidate_stops_later_factories_and_all_admission() {
    for change in [
        Change::Width,
        Change::Name,
        Change::Shape,
        Change::Cost,
        Change::Candidate,
    ] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.change = Some(change);
        let error = lower(&mut adapter, LIMITS).err().unwrap();
        match change {
            Change::Width => assert!(matches!(
                error,
                RepeatSequenceSourceError::Width { iteration: 2, .. }
            )),
            Change::Name => assert!(matches!(
                error,
                RepeatSequenceSourceError::NamesChanged {
                    iteration: 2,
                    member: 0,
                    ..
                }
            )),
            Change::Shape => assert!(matches!(
                error,
                RepeatSequenceSourceError::Repeat(RepeatSourceError::ShapeChanged {
                    iteration: 2,
                    ..
                })
            )),
            Change::Cost => assert!(matches!(
                error,
                RepeatSequenceSourceError::Repeat(RepeatSourceError::CostChanged {
                    iteration: 2,
                    ..
                })
            )),
            Change::Candidate => assert!(matches!(
                error,
                RepeatSequenceSourceError::Candidate {
                    iteration: 2,
                    member: 0,
                    ..
                }
            )),
            _ => panic!(),
        }
        assert_eq!(
            adapter.events,
            [
                (RepeatSourcePhase::Lower, 1, 0),
                (RepeatSourcePhase::Materialize, 2, 0)
            ]
        );
        assert!(adapter.scope.is_empty());
        assert_eq!(
            adapter.drops.get(),
            if matches!(change, Change::Shape) {
                9
            } else {
                8
            }
        );
    }
}

#[test]
fn equal_shape_cost_and_result_tags_cannot_replace_exact_native_identity() {
    for change in [Change::Operation, Change::Result] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.change = Some(change);
        assert!(matches!(
            lower(&mut adapter, LIMITS),
            Err(RepeatSequenceSourceError::Admission {
                iteration: 2,
                member: 0,
                error: PrivateError("wrong native identity"),
                ..
            })
        ));
        assert_eq!(
            adapter.events.last(),
            Some(&(RepeatSourcePhase::Admit, 2, 0))
        );
        assert_eq!(adapter.drops.get(), 12);
    }
}

#[test]
fn native_failures_keep_indices_spans_payload_and_release_all_parts_without_retry() {
    for phase in [
        (RepeatSourcePhase::Lower, 1, 0),
        (RepeatSourcePhase::Materialize, 2, 0),
        (RepeatSourcePhase::Materialize, 3, 0),
        (RepeatSourcePhase::Admit, 1, 1),
        (RepeatSourcePhase::Admit, 2, 1),
        (RepeatSourcePhase::Admit, 3, 1),
    ] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.fail = Some(phase);
        let error = lower(&mut adapter, LIMITS).err().unwrap();
        assert!(!format!("{error} {error:?}").contains("secret"));
        assert!(std::error::Error::source(&error).is_none());
        match error {
            RepeatSequenceSourceError::Repeat(RepeatSourceError::Native {
                iteration,
                phase: actual,
                span,
                error: PrivateError(secret),
            }) => {
                assert_eq!((actual, iteration, 0), phase);
                assert_eq!(span, adapter.body().span);
                assert_eq!(secret, "secret credential");
            }
            RepeatSequenceSourceError::Admission {
                iteration,
                member,
                span,
                error: PrivateError(secret),
            } => {
                assert_eq!((RepeatSourcePhase::Admit, iteration, member), phase);
                assert_eq!(span, adapter.body().span);
                assert_eq!(secret, "secret credential");
            }
            _ => panic!(),
        }
        assert_eq!(adapter.events.last(), Some(&phase));
        assert_eq!(
            adapter.drops.get(),
            match phase.0 {
                RepeatSourcePhase::Lower => 0,
                RepeatSourcePhase::Materialize => (phase.1 - 1) * 4,
                RepeatSourcePhase::Admit => 12,
            }
        );
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn native_unwind_releases_consumed_instances_and_restores_frames_without_replay() {
    for phase in [
        (RepeatSourcePhase::Materialize, 3, 0),
        (RepeatSourcePhase::Admit, 2, 1),
    ] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.fail = Some(phase);
        adapter.unwind = true;
        assert!(catch_unwind(AssertUnwindSafe(|| lower(&mut adapter, LIMITS))).is_err());
        assert_eq!(adapter.events.last(), Some(&phase));
        assert_eq!(
            adapter.drops.get(),
            if phase.0 == RepeatSourcePhase::Materialize {
                8
            } else {
                12
            }
        );
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn late_native_cost_error_or_unwind_never_admits_partial_instances() {
    for unwind in [false, true] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.cost_fail = Some(3);
        adapter.unwind = unwind;
        if unwind {
            assert!(catch_unwind(AssertUnwindSafe(|| lower(&mut adapter, LIMITS))).is_err());
        } else {
            let error = lower(&mut adapter, LIMITS).err().unwrap();
            assert!(!format!("{error:?}").contains("credential"));
            assert!(matches!(
                error,
                RepeatSequenceSourceError::Repeat(RepeatSourceError::Cost {
                    iteration: 2,
                    error: SourceCostError::Observation {
                        host_index: 0,
                        error: PrivateError("private cost credential")
                    }
                })
            ));
        }
        assert_eq!(adapter.events.len(), 2);
        assert_eq!(adapter.drops.get(), 8);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn reference_nested_repeat_names_wire_authority_and_expansion_diagnostics_remain_compatible() {
    for body in [
        "repeat(times: 3, body: seq(one: ui.focus(node_id: node), two: runtime.list()))",
        "repeat(times: 2, body: repeat(times: 2, body: ui.focus(node_id: node)))",
        "seq(prefix: repeat(times: 2, body: seq(one: seq(two: ui.focus(node_id: node)), three: runtime.list())))",
        "repeat(times: 2, body: seq(one: ui.focus(node_id: \"a\"), two: runtime.list()))",
    ] {
        let program = leselang_hir::lower(&parse(&format!(
            "fn main() = bind(node: \"a\", body: {body})"
        )))
        .unwrap();
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
    for (body, message) in [
        (
            "repeat(times: 33, body: seq(one: ui.focus(node_id: node), two: runtime.list()))",
            "expanded control flow exceeds 64 effects",
        ),
        (
            "repeat(times: 2, body: all(one: ui.focus(node_id: node), two: runtime.list()))",
            "all cannot be nested inside sequential control flow",
        ),
    ] {
        let errors = leselang_hir::lower(&parse(&format!(
            "fn main() = bind(node: \"a\", body: {body})"
        )))
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.code == "LSH1301" && error.message == message)
        );
    }
}
